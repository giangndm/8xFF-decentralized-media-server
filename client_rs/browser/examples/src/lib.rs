//! Pure Rust browser examples for the generic Atm0s client.
mod app;
mod capture;
mod harness;
mod media_evidence;
mod rendering;
mod scenario_flow;
mod scenarios;
mod session;
mod tick;
mod ui;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&format!("client-rs panic: {info}").into())));
    let document = web_sys::window().and_then(|window| window.document()).ok_or_else(|| JsValue::from_str("document unavailable"))?;
    ui::mount(&document);
    app::install(document)
}
