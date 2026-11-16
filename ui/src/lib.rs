//! Posthaste's interface. One WebAssembly module serves every window that
//! has moved to Rust; the window's label decides which one it shows.
//!
//! How the crate is laid out, and the one rule each part follows:
//!
//! | Module | Holds | Rule |
//! |---|---|---|
//! | `logic` | decisions as plain functions | no browser, no framework; tested with `cargo test` |
//! | `ipc` | every command and event | the only module that touches Tauri |
//! | `time` | the clock, the time zone, date formatting | the only module that reads them from the browser |
//! | `state` | settings, language, look, shared by windows | the only writer of `<html>` attributes |
//! | `components` | controls that know nothing about notes | no `ipc` imports |
//! | `windows` | one module per window | composes the layers above |

pub mod logic;

#[cfg(target_arch = "wasm32")]
mod components;
#[cfg(target_arch = "wasm32")]
mod draft_sync;
#[cfg(target_arch = "wasm32")]
mod ipc;
#[cfg(target_arch = "wasm32")]
mod state;
#[cfg(target_arch = "wasm32")]
mod time;
#[cfg(target_arch = "wasm32")]
mod windows;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    windows::mount();
}
