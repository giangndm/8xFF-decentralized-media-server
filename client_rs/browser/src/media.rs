use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{MediaStream, MediaStreamConstraints, MediaStreamTrack};

#[derive(Debug, Clone)]
pub struct BrowserTrack(pub MediaStreamTrack);

#[derive(Debug, Clone)]
pub struct BrowserStream(pub MediaStream);

/// Request camera/microphone capture. The returned stream is owned by the
/// application; SDK sender bindings borrow its tracks.
pub async fn get_user_media(audio: bool, video: bool) -> Result<BrowserStream, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let devices = window.navigator().media_devices()?;
    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&audio.into());
    constraints.set_video(&video.into());
    let stream = JsFuture::from(devices.get_user_media_with_constraints(&constraints)?).await?.dyn_into::<MediaStream>()?;
    Ok(BrowserStream(stream))
}
