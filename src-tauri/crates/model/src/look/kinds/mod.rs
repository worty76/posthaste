//! Layer kinds. One file per kind: its config shape, its limits and the
//! regions it may sit in. Adding a kind also means a `Known` variant in
//! `layer.rs` and a check arm in core's `look/scene.rs` — the compiler
//! finds both once the variant exists.

pub mod emoji;
pub mod image;
