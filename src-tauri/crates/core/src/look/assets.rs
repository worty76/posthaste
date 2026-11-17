//! Look image assets: bytes in SQLite, addressed by the SHA-256 of their
//! content, accepted only after the file header and the size limits
//! check out. Storing the same bytes twice is free: the second store
//! finds the first row and inserts nothing.

use sha2::{Digest, Sha256};

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use crate::look::{AssetRef, LookDocument};

/// The biggest image this build stores.
pub const MAX_ASSET_BYTES: usize = 2 * 1024 * 1024;
/// The biggest width or height, read from the file header.
pub const MAX_ASSET_SIDE: u32 = 2560;
/// The most assets one look may list.
pub const MAX_ASSETS_PER_LOOK: usize = 8;
/// The most stored image bytes one look may claim.
pub const MAX_ASSET_TOTAL_BYTES: usize = 10 * 1024 * 1024;
/// How long an asset nobody references is kept before the launch purge.
pub const ASSET_GRACE_MS: i64 = 24 * 3600 * 1000;

/// What the file header says about an image.
#[derive(Debug)]
pub struct ImageInfo {
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Reads dimensions from the file header of a PNG, JPEG or WebP image.
/// The name is never consulted, and nothing past the header is parsed.
pub fn sniff_image(bytes: &[u8]) -> Result<ImageInfo> {
    let truncated = || {
        CoreError::InvalidAsset(
            "That image file is cut off or not a PNG, JPEG or WebP image.".into(),
        )
    };
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        // Signature, IHDR length, "IHDR", width, height.
        if bytes.len() < 33 || &bytes[12..16] != b"IHDR" {
            return Err(truncated());
        }
        return Ok(ImageInfo {
            mime: "image/png",
            width: u32::from_be_bytes(bytes[16..20].try_into().expect("4 bytes")),
            height: u32::from_be_bytes(bytes[20..24].try_into().expect("4 bytes")),
        });
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 1 < bytes.len() {
            if bytes[i] != 0xFF {
                return Err(truncated());
            }
            while i < bytes.len() && bytes[i] == 0xFF {
                i += 1;
            }
            let Some(&marker) = bytes.get(i) else {
                break;
            };
            // A frame header carries the dimensions. DHT (C4), JPG (C8)
            // and DAC (CC) sit in the same range without being frames.
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                if i + 8 > bytes.len() {
                    return Err(truncated());
                }
                return Ok(ImageInfo {
                    mime: "image/jpeg",
                    height: u16::from_be_bytes([bytes[i + 4], bytes[i + 5]]) as u32,
                    width: u16::from_be_bytes([bytes[i + 6], bytes[i + 7]]) as u32,
                });
            }
            if marker == 0xDA || marker == 0xD9 {
                break; // Scan data or end of image before any frame header.
            }
            if i + 3 > bytes.len() {
                break;
            }
            let len = u16::from_be_bytes([bytes[i + 1], bytes[i + 2]]) as usize;
            if len < 2 {
                return Err(truncated());
            }
            i += len + 1;
        }
        return Err(truncated());
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        let (width, height) = match &bytes[12..16] {
            b"VP8 " => {
                if bytes.len() < 30 {
                    return Err(truncated());
                }
                (
                    u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3FFF,
                    u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3FFF,
                )
            }
            b"VP8L" => {
                if bytes.len() < 25 || bytes[20] != 0x2F {
                    return Err(truncated());
                }
                let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
                ((bits & 0x3FFF) + 1, (bits >> 14) + 1)
            }
            b"VP8X" => {
                if bytes.len() < 30 {
                    return Err(truncated());
                }
                (u24_le(&bytes[24..27]) + 1, u24_le(&bytes[27..30]) + 1)
            }
            _ => return Err(truncated()),
        };
        return Ok(ImageInfo {
            mime: "image/webp",
            width,
            height,
        });
    }
    Err(CoreError::InvalidAsset(
        "Look images are PNG, JPEG or WebP files. That file is neither.".into(),
    ))
}

fn u24_le(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

fn asset_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

fn asset_ref(id: String, info: &ImageInfo, bytes: usize) -> AssetRef {
    AssetRef {
        id,
        asset_type: info.mime.to_string(),
        bytes: bytes as u64,
        width: info.width,
        height: info.height,
        extra: Default::default(),
    }
}

impl Db {
    /// Checks, hashes and stores an image, and answers the document's
    /// claim about it. The same bytes always answer the same asset.
    pub fn store_look_asset(&mut self, bytes: &[u8]) -> Result<AssetRef> {
        self.store_look_asset_at(bytes, now_ms())
    }

    pub fn store_look_asset_at(&mut self, bytes: &[u8], now: i64) -> Result<AssetRef> {
        if bytes.is_empty() {
            return Err(CoreError::InvalidAsset("That image file is empty.".into()));
        }
        // The size check comes first: a huge blob is refused before any
        // of it is hashed or parsed.
        if bytes.len() > MAX_ASSET_BYTES {
            return Err(CoreError::InvalidAsset(format!(
                "That image is {:.1} MB. Look images are up to 2 MB.",
                bytes.len() as f64 / (1024.0 * 1024.0)
            )));
        }
        let info = sniff_image(bytes)?;
        if info.width == 0 || info.height == 0 {
            return Err(CoreError::InvalidAsset("That image has no pixels.".into()));
        }
        if info.width > MAX_ASSET_SIDE || info.height > MAX_ASSET_SIDE {
            return Err(CoreError::InvalidAsset(format!(
                "That image is {}×{}. Look images are at most {}×{}.",
                info.width, info.height, MAX_ASSET_SIDE, MAX_ASSET_SIDE
            )));
        }
        let id = asset_id(bytes);
        self.conn().execute(
            "INSERT OR IGNORE INTO look_assets (id, mime, size, width, height, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                info.mime,
                bytes.len() as i64,
                info.width as i64,
                info.height as i64,
                bytes,
                now
            ],
        )?;
        Ok(asset_ref(id, &info, bytes.len()))
    }

    /// The stored bytes and their MIME type. `NotFound` for an id that is
    /// not stored.
    pub fn look_asset_bytes(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let (mime, data): (String, Vec<u8>) = self
            .conn()
            .query_row(
                "SELECT mime, data FROM look_assets WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| CoreError::NotFound)?;
        Ok((mime, data))
    }

    /// Deletes stored assets that are old enough and that no look's
    /// `assets` list names. One unreadable look row stops the purge: it
    /// might reference anything. Answers how many rows went.
    pub fn purge_unused_look_assets(&mut self, now: i64) -> Result<usize> {
        let mut referenced = std::collections::HashSet::new();
        {
            let mut stmt = self.conn().prepare("SELECT doc FROM looks")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for json in rows {
                let doc = LookDocument::parse_stored(&json?)?;
                for asset in doc_assets(&doc) {
                    referenced.insert(asset);
                }
            }
        }
        let cutoff = now - ASSET_GRACE_MS;
        let stale: Vec<String> = {
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM look_assets WHERE created_at < ?1")?;
            let rows = stmt.query_map([cutoff], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let tx = self.conn_mut().transaction()?;
        let mut removed = 0;
        for id in &stale {
            if !referenced.contains(id) {
                tx.execute("DELETE FROM look_assets WHERE id = ?1", [id])?;
                removed += 1;
            }
        }
        tx.commit()?;
        Ok(removed)
    }
}

/// The asset ids a stored document lists, whatever its version.
fn doc_assets(doc: &LookDocument) -> Vec<String> {
    match doc {
        LookDocument::V1(_) => Vec::new(),
        LookDocument::V2(d) => d.assets.iter().map(|a| a.id.clone()).collect(),
    }
}
