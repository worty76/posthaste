//! The raw bindings to `window.__TAURI__`. Private to `ipc`: nothing
//! outside names a command or an event as a string.

use posthaste_model::CommandError;
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke_with(
        command: &str,
        body: JsValue,
        options: JsValue,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    pub(super) fn tauri_listen(
        event: &str,
        handler: &Closure<dyn FnMut(JsValue)>,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "window"], js_name = getCurrentWindow, catch)]
    fn tauri_current_window() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "webview"], js_name = getCurrentWebview, catch)]
    fn tauri_current_webview() -> Result<JsValue, JsValue>;
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
    decode(invoke_raw(command, args).await?)
}

/// Runs a command whose answer is bytes, not JSON.
pub(super) async fn invoke_for_bytes<A: Serialize>(
    command: &str,
    args: &A,
) -> Result<Vec<u8>, CommandError> {
    let answer = invoke_raw(command, args).await?;
    // The shell answers with an ArrayBuffer; a typed array is copied as is.
    Ok(js_sys::Uint8Array::new(&answer).to_vec())
}

/// Runs a command whose body is bytes. What would be its arguments travel
/// as headers, percent-encoded because a header cannot hold every character.
pub(super) async fn invoke_with_bytes<R: DeserializeOwned>(
    command: &str,
    bytes: &[u8],
    headers: &[(&str, &str)],
) -> Result<R, CommandError> {
    let encoded = js_sys::Object::new();
    for (name, value) in headers {
        let value = js_sys::encode_uri_component(value);
        let _ = js_sys::Reflect::set(&encoded, &JsValue::from_str(name), &value.into());
    }
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &JsValue::from_str("headers"), &encoded);
    let body = js_sys::Uint8Array::from(bytes);
    let answer = tauri_invoke_with(command, body.into(), options.into())
        .await
        .map_err(thrown_error)?;
    decode(answer)
}

async fn invoke_raw<A: Serialize>(command: &str, args: &A) -> Result<JsValue, CommandError> {
    // JSON-compatible: `None` becomes null and maps become plain objects,
    // which is what the shell's serde expects.
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    let args = args
        .serialize(&serializer)
        .map_err(|_| unknown_error(JsValue::NULL))?;
    tauri_invoke(command, args).await.map_err(thrown_error)
}

fn decode<R: DeserializeOwned>(answer: JsValue) -> Result<R, CommandError> {
    serde_wasm_bindgen::from_value(answer).map_err(|_| CommandError {
        code: "bad_response".into(),
        message: String::new(),
    })
}

fn thrown_error(thrown: JsValue) -> CommandError {
    serde_wasm_bindgen::from_value::<CommandError>(thrown.clone())
        .unwrap_or_else(|_| unknown_error(thrown))
}

/// Asks the webview to call `handler` for every drag over the window and
/// every drop on it. Resolves to the function that stops it.
pub(super) fn tauri_on_drag_drop(
    handler: &Closure<dyn FnMut(JsValue)>,
) -> Result<js_sys::Promise, JsValue> {
    let webview = tauri_current_webview()?;
    let listen: js_sys::Function =
        js_sys::Reflect::get(&webview, &JsValue::from_str("onDragDropEvent"))?.dyn_into()?;
    listen.call1(&webview, handler.as_ref())?.dyn_into()
}

/// A failure that is not the shell's `{ code, message }`: a plain string,
/// or nothing usable at all.
fn unknown_error(thrown: JsValue) -> CommandError {
    CommandError {
        code: "unknown".into(),
        message: thrown.as_string().unwrap_or_default(),
    }
}
