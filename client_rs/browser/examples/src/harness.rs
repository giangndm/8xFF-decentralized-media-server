//! Stable DOM bridge used by the Rust example session.
use wasm_bindgen::prelude::*;
use web_sys::{console, Document, Element, HtmlInputElement};

pub fn append_log(log: &Element, value: &str) {
    if let Some(input) = log.dyn_ref::<HtmlInputElement>() {
        input.set_value(&format!("{}\n{}", input.value(), value));
    } else {
        let current = log.text_content().unwrap_or_default();
        log.set_text_content(Some(&format!("{}\n{}", current, value)));
    }
    console::log_1(&JsValue::from_str(value));
}

pub fn set_assert(document: &Document, name: &str, state: &str) {
    if let Some(item) = document.query_selector(&format!("[data-assert='{name}']")).ok().flatten() {
        item.set_text_content(Some(&format!("{name}: {state}")));
        let _ = item.set_attribute("data-state", state);
    }
}

pub fn now_ms() -> u64 {
    web_sys::window().and_then(|window| window.performance()).map(|performance| performance.now() as u64).unwrap_or(0)
}

pub fn input_value(element: &Element) -> String {
    element.dyn_ref::<HtmlInputElement>().map(|input| input.value()).unwrap_or_default()
}
