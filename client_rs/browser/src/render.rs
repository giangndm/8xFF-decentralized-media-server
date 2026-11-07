use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlMediaElement, MediaStream};

/// Bind a stream to a media element. Autoplay rejection is returned.
pub async fn bind_and_play(element: &HtmlMediaElement, stream: &MediaStream) -> Result<(), JsValue> {
    element.set_src_object(Some(stream));
    JsFuture::from(element.play()?).await.map(|_| ())
}

pub fn unbind(element: &HtmlMediaElement) {
    element.set_src_object(None);
}
