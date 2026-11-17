//! Look files: a look and its images in one `.phlook` file — one JSON
//! document, base64url images, nothing else. Export writes only what the
//! look itself lists; import refuses anything it cannot verify, and
//! writes nothing until every check has passed.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::db::{now_ms, Db};
use crate::error::{CoreError, Result};
use crate::look::assets::sniff_image;
use crate::look::code::{b64_decode, b64_encode};
use crate::look::store::LookSummary;
use crate::look::{LookDocExt, LookDocV2, LookDocV2Ext};

/// The largest look file this build reads, checked before parsing.
pub const MAX_PACKAGE_BYTES: usize = 12 * 1024 * 1024;

/// The format string inside a look file.
const PACKAGE_FORMAT: &str = "posthaste-look-file";
const PACKAGE_VERSION: u32 = 1;

/// A look read from a file: the document and every image it lists, with
/// each image's document claims already verified against its bytes.
#[derive(Debug)]
pub struct PendingLook {
    pub doc: LookDocV2,
    pub assets: Vec<(crate::look::AssetRef, Vec<u8>)>,
}

/// Builds a look file. `bytes_of` answers the stored bytes for one of
/// the look's listed assets; a listed image with no stored bytes is
/// refused, so a shared file never carries a dangling reference.
pub fn write_package(
    doc: &LookDocV2,
    bytes_of: impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<String> {
    doc.validate()?;
    let mut assets = Map::new();
    for asset in &doc.assets {
        let bytes = bytes_of(&asset.id).map_err(|_| {
            CoreError::InvalidAsset(format!(
                "The image \"{}\" is missing. Re-add it before exporting.",
                asset.id
            ))
        })?;
        assets.insert(asset.id.clone(), Value::String(b64_encode(&bytes)));
    }
    let file = json!({
        "format": PACKAGE_FORMAT,
        "version": PACKAGE_VERSION,
        "look": serde_json::to_value(doc).unwrap_or_default(),
        "assets": assets,
    });
    Ok(serde_json::to_string(&file).unwrap_or_default())
}

/// Reads a look file. Every check runs before anything is returned:
/// size, format, the look itself, and every image against its document
/// claims and its own digest.
pub fn read_package(json: &str) -> Result<PendingLook> {
    if json.len() > MAX_PACKAGE_BYTES {
        return Err(CoreError::InvalidAsset(format!(
            "That look file is too large. Look files are up to {} MB.",
            MAX_PACKAGE_BYTES / (1024 * 1024)
        )));
    }
    let file: Value = serde_json::from_str(json)
        .map_err(|e| CoreError::InvalidAsset(format!("That isn't a Posthaste look file: {e}")))?;
    if file["format"] != json!(PACKAGE_FORMAT) || file["version"] != json!(PACKAGE_VERSION) {
        return Err(CoreError::InvalidAsset(
            "That file isn't a Posthaste look file.".into(),
        ));
    }
    let doc: LookDocV2 = serde_json::from_value(file["look"].clone())
        .map_err(|e| CoreError::InvalidAsset(format!("That isn't a Posthaste look file: {e}")))?;
    doc.validate()?;

    let listed = &doc.assets;
    let carried = file["assets"]
        .as_object()
        .ok_or_else(|| CoreError::InvalidAsset("That look file is missing its images.".into()))?;

    // Anything the file carries that the look does not list is refused:
    // extra bytes are either a mistake or a way to smuggle content in.
    for key in carried.keys() {
        if !listed.iter().any(|asset| &asset.id == key) {
            return Err(CoreError::InvalidAsset(format!(
                "That look file carries an image the look does not use."
            )));
        }
    }

    let mut assets = Vec::new();
    for asset in listed {
        let Some(encoded) = carried.get(&asset.id).and_then(Value::as_str) else {
            return Err(CoreError::InvalidAsset(format!(
                "That look file is missing the image \"{}\".",
                asset.id
            )));
        };
        let bytes = b64_decode(encoded).ok_or_else(|| {
            CoreError::InvalidAsset(format!("The image \"{}\" is unreadable.", asset.id))
        })?;
        // The bytes must hash to the id they travel under.
        let digest = Sha256::digest(&bytes);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        if format!("sha256:{hex}") != asset.id {
            return Err(CoreError::InvalidAsset(
                "That look file changed on the way: an image does not match its id.".into(),
            ));
        }
        // And they must be the image the document claims.
        let info = sniff_image(&bytes)?;
        if info.mime != asset.asset_type
            || bytes.len() as u64 != asset.bytes
            || info.width != asset.width
            || info.height != asset.height
        {
            return Err(CoreError::InvalidAsset(format!(
                "The image \"{}\" does not match what the look says about it.",
                asset.id
            )));
        }
        assets.push((asset.clone(), bytes));
    }

    Ok(PendingLook { doc, assets })
}

impl Db {
    /// Stores a read look file: every image and the look itself, in one
    /// transaction, so a half-installed file can never exist.
    pub fn install_package(&mut self, pending: PendingLook) -> Result<LookSummary> {
        pending.doc.validate()?;
        let json = pending.doc.to_json();
        let now = now_ms();
        let id = format!("user:{}", uuid::Uuid::now_v7());
        let tx = self.conn_mut().transaction()?;
        for (asset, bytes) in &pending.assets {
            tx.execute(
                "INSERT OR IGNORE INTO look_assets (id, mime, size, width, height, data, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    asset.id,
                    asset.asset_type,
                    bytes.len() as i64,
                    asset.width as i64,
                    asset.height as i64,
                    bytes,
                    now
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO looks (id, doc, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![id, json, now],
        )?;
        tx.commit()?;

        let theme = pending.doc.v1_view();
        Ok(LookSummary {
            id,
            name: theme.name.clone(),
            author: theme.author.clone(),
            builtin: false,
            base: theme.base,
            preview: theme.preview(),
        })
    }
}
