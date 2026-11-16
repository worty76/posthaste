//! The raw bindings to `window.__TAURI__`. Private to `ipc`: nothing
//! outside names a command or an event as a string.

use posthaste_model::CommandError;
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    pub(super) fn tauri_listen(
        event: &str,
        handler: &Closure<dyn FnMut(JsValue)>,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "window"], js_name = getCurrentWindow, catch)]
    fn tauri_current_window() -> Result<JsValue, JsValue>;
}

/// The label the shell gave this window ("capture", "today", …).
pub fn window_label() -> Option<String> {
    let window = tauri_current_window().ok()?;
    js_sys::Reflect::get(&window, &JsValue::from_str("label"))
        .ok()?
        .as_string()
}

/// Arguments for a command that takes none.
#[derive(Serialize)]
pub(super) struct NoArgs {}

/// Runs a command and decodes its answer.
pub(super) async fn invoke<A, R>(command: &str, args: &A) -> Result<R, CommandError>
where
    A: Serialize,
    R: DeserializeOwned,
{
    // JSON-compatible: `None` becomes null and maps become plain objects,
    // which is what the shell's serde expects.
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    let args = args
        .serialize(&serializer)
        .map_err(|_| unknown_error(JsValue::NULL))?;
    match tauri_invoke(command, args).await {
        Ok(value) => serde_wasm_bindgen::from_value(value).map_err(|_| CommandError {
            code: "bad_response".into(),
            message: String::new(),
        }),
        Err(thrown) => Err(
            serde_wasm_bindgen::from_value::<CommandError>(thrown.clone())
                .unwrap_or_else(|_| unknown_error(thrown)),
        ),
    }
}

/// A failure that is not the shell's `{ code, message }`: a plain string,
/// or nothing usable at all.
fn unknown_error(thrown: JsValue) -> CommandError {
    CommandError {
        code: "unknown".into(),
        message: thrown.as_string().unwrap_or_default(),
    }
}
