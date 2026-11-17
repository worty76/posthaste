//! The asset list: one entry per file a scene uses, addressed by content.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One file a scene uses, addressed by the SHA-256 of its stored bytes.
/// The bytes themselves are storage's business (a later phase); this is
/// only the document's claim about what exists.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AssetRef {
    /// `sha256:` followed by the lowercase hex digest of the stored bytes.
    pub id: String,
    /// The file's MIME type, e.g. `image/png`.
    #[serde(rename = "type")]
    pub asset_type: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
