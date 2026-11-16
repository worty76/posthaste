//! Files attached to a note: how big, what kind, what to call them, and
//! how their bytes become something an `<img>` can show.

use posthaste_model::MAX_ATTACHMENTS_PER_NOTE;

/// "860 B", "86 KB", "2.4 MB". Units are the same in every language.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * KB;
    if bytes < KB {
        return format!("{bytes} B");
    }
    if bytes < MB {
        return format!("{} KB", (bytes + KB / 2) / KB);
    }
    // One decimal under 10 MB, where it still tells files apart.
    let tenths = (bytes * 10 + MB / 2) / MB;
    if tenths < 100 {
        format!("{}.{} MB", tenths / 10, tenths % 10)
    } else {
        format!("{} MB", (bytes + MB / 2) / MB)
    }
}

/// What a tile draws for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Image,
    Pdf,
    Text,
    Other,
}

impl Kind {
    pub fn of(mime: &str) -> Kind {
        if mime.starts_with("image/") {
            Kind::Image
        } else if mime == "application/pdf" {
            Kind::Pdf
        } else if mime.starts_with("text/") || mime == "application/json" {
            Kind::Text
        } else {
            Kind::Other
        }
    }

    /// The value of `data-kind` on the tile.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Pdf => "pdf",
            Kind::Text => "text",
            Kind::Other => "other",
        }
    }
}

/// A short label for a tile without a thumbnail: the file's extension when
/// it has a short one, else its kind.
pub fn type_label(name: &str, mime: &str) -> String {
    if let Some((stem, extension)) = name.rsplit_once('.') {
        let length = extension.chars().count();
        if !stem.is_empty() && (1..=5).contains(&length) {
            return extension.to_uppercase();
        }
    }
    match Kind::of(mime) {
        Kind::Other => "FILE".to_owned(),
        kind => kind.as_str().to_uppercase(),
    }
}

/// Whether a note holding `existing` files can take another. Asked before
/// the shell is, so a full note says so without opening a file dialog.
pub fn has_room(existing: usize) -> bool {
    existing < MAX_ATTACHMENTS_PER_NOTE
}

/// Whether a pasted image arrived without a name worth keeping. Browsers
/// call every clipboard image "image.png".
pub fn is_unnamed_paste(name: &str) -> bool {
    name.is_empty() || name == "image.png"
}

/// The extension for a pasted image of this type.
pub fn image_extension(mime: &str) -> String {
    match mime {
        "image/jpeg" => "jpg".to_owned(),
        _ => mime
            .split_once('/')
            .map_or("png", |(_, subtype)| subtype)
            .replace("+xml", ""),
    }
}

/// `data:` URL for image bytes. `blob:` URLs are not allowed by the CSP.
pub fn data_url(bytes: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sextet = |value: u32, shift: u32| char::from(ALPHABET[((value >> shift) & 63) as usize]);
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk.iter().enumerate().fold(0u32, |all, (i, byte)| {
            all | u32::from(*byte) << (16 - 8 * i)
        });
        out.push(sextet(value, 18));
        out.push(sextet(value, 12));
        out.push(if chunk.len() > 1 {
            sextet(value, 6)
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            sextet(value, 0)
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_written_in_the_largest_unit_that_fits() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(860), "860 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(88_000), "86 KB");
        assert_eq!(format_bytes(2_516_582), "2.4 MB");
        assert_eq!(format_bytes(15 * 1024 * 1024), "15 MB");
    }

    #[test]
    fn a_files_kind_comes_from_its_type() {
        assert_eq!(Kind::of("image/png"), Kind::Image);
        assert_eq!(Kind::of("application/pdf"), Kind::Pdf);
        assert_eq!(Kind::of("text/plain"), Kind::Text);
        assert_eq!(Kind::of("application/json"), Kind::Text);
        assert_eq!(Kind::of("application/zip"), Kind::Other);
    }

    #[test]
    fn a_tile_is_labelled_by_extension_or_kind() {
        assert_eq!(type_label("notes.md", "text/markdown"), "MD");
        assert_eq!(type_label("photo.JPEG", "image/jpeg"), "JPEG");
        assert_eq!(type_label("README", "text/plain"), "TEXT");
        assert_eq!(type_label("archive", "application/zip"), "FILE");
        // Not an extension: too long, or nothing before the dot.
        assert_eq!(type_label("report.final-draft", "application/pdf"), "PDF");
        assert_eq!(type_label(".env", "application/octet-stream"), "FILE");
        assert_eq!(type_label("odd.", "application/octet-stream"), "FILE");
    }

    #[test]
    fn a_note_takes_files_until_it_is_full() {
        assert!(has_room(0));
        assert!(has_room(MAX_ATTACHMENTS_PER_NOTE - 1));
        assert!(!has_room(MAX_ATTACHMENTS_PER_NOTE));
    }

    #[test]
    fn pasted_images_get_an_extension_from_their_type() {
        assert_eq!(image_extension("image/jpeg"), "jpg");
        assert_eq!(image_extension("image/png"), "png");
        assert_eq!(image_extension("image/svg+xml"), "svg");
        assert_eq!(image_extension("nonsense"), "png");
        assert!(is_unnamed_paste("image.png"));
        assert!(is_unnamed_paste(""));
        assert!(!is_unnamed_paste("diagram.png"));
    }

    #[test]
    fn bytes_become_a_data_url() {
        assert_eq!(data_url(b"", "image/png"), "data:image/png;base64,");
        assert_eq!(data_url(b"f", "image/png"), "data:image/png;base64,Zg==");
        assert_eq!(data_url(b"fo", "image/png"), "data:image/png;base64,Zm8=");
        assert_eq!(data_url(b"foo", "image/png"), "data:image/png;base64,Zm9v");
        assert_eq!(
            data_url(&[0xff, 0xfe, 0xfd, 0x00], "image/gif"),
            "data:image/gif;base64,//79AA=="
        );
    }
}
