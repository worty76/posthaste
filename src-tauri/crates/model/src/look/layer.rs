//! The layer envelope: one shape that parses whatever the kind is, and a
//! separate step that turns it into a typed layer.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::look::kinds::emoji::EmojiLayer;
use crate::look::kinds::image::ImageLayer;

/// The envelope every scene layer is written in. It never fails to parse
/// because of a new kind: unknown kinds and unknown fields land in
/// `config` and `extra` verbatim, and a known kind with a config this
/// build cannot read stays a value until [`LayerRecord::read`] tells the
/// checker to refuse it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LayerRecord {
    pub kind: String,
    pub v: u32,
    /// Off layers are kept, not dropped, so a toggle survives a downgrade.
    /// Written only when off: a default true would grow every document on
    /// a round trip.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub on: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub config: Value,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The layer's config parsed into a type this build knows. Untagged: it
/// serializes as the typed config itself.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(untagged)]
pub enum Known {
    Image(ImageLayer),
    Emoji(EmojiLayer),
}

/// How this build can read a layer. A config this build should understand
/// but cannot is distinct from a kind it predates: the first is a refused
/// document, the second is kept for a newer reader.
#[derive(Clone, PartialEq, Debug)]
pub enum Read {
    Known(Known),
    Future,
    Malformed,
}

/// The envelope's default for `on`.
fn yes() -> bool {
    true
}

/// The skip condition for `on`.
fn is_true(on: &bool) -> bool {
    *on
}

/// True when any string anywhere in the value equals `needle`.
fn value_mentions(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(s) => s == needle,
        Value::Array(items) => items.iter().any(|v| value_mentions(v, needle)),
        Value::Object(map) => map.values().any(|v| value_mentions(v, needle)),
        _ => false,
    }
}

impl LayerRecord {
    /// How this build reads the layer. Adding a kind means a branch here,
    /// a `Known` variant, and an arm in every check and renderer — the
    /// compiler requires them all.
    pub fn read(&self) -> Read {
        if self.kind == ImageLayer::KIND && self.v <= ImageLayer::VERSION {
            return match serde_json::from_value::<ImageLayer>(self.config.clone()) {
                Ok(image) => Read::Known(Known::Image(image)),
                Err(_) => Read::Malformed,
            };
        }
        if self.kind == EmojiLayer::KIND && self.v <= EmojiLayer::VERSION {
            return match serde_json::from_value::<EmojiLayer>(self.config.clone()) {
                Ok(emoji) => Read::Known(Known::Emoji(emoji)),
                Err(_) => Read::Malformed,
            };
        }
        Read::Future
    }

    /// The typed layer, when this build can draw it: `None` for a kind,
    /// version or config it does not understand.
    pub fn known(&self) -> Option<Known> {
        match self.read() {
            Read::Known(known) => Some(known),
            Read::Future | Read::Malformed => None,
        }
    }

    /// A fresh record for a typed layer, as an editor would add one.
    pub fn from_known(known: &Known) -> LayerRecord {
        let (kind, version) = match known {
            Known::Image(_) => (ImageLayer::KIND, ImageLayer::VERSION),
            Known::Emoji(_) => (EmojiLayer::KIND, EmojiLayer::VERSION),
        };
        LayerRecord {
            kind: kind.into(),
            v: version,
            on: true,
            config: serde_json::to_value(known).unwrap_or_default(),
            extra: Map::new(),
        }
    }

    /// Writes the typed layer's config back, keeping `extra`, `kind` and
    /// `v` — so keys the typed layer carries in its own `extra` survive
    /// an edit.
    pub fn set_known(&mut self, known: &Known) {
        self.config = serde_json::to_value(known).unwrap_or_default();
    }

    /// True when any string anywhere in `config` or `extra` equals
    /// `needle` — how the editor finds every layer that names an asset.
    pub fn mentions(&self, needle: &str) -> bool {
        value_mentions(&self.config, needle)
            || self.extra.values().any(|v| value_mentions(v, needle))
    }
}
