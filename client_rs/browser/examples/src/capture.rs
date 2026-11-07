use atm0s_media_client_browser::BrowserStream;
use wasm_bindgen::JsCast;

/// Example captures belong to the application, not the SDK's borrowed registry.
pub struct Capture(pub BrowserStream);
impl Capture {
    pub fn stopped(&self) -> bool {
        self.0
             .0
            .get_tracks()
            .iter()
            .all(|track| js_sys::Reflect::get(&track, &"readyState".into()).ok().and_then(|state| state.as_string()).as_deref() == Some("ended"))
    }
    pub fn stop(&self) {
        for track in self.0 .0.get_tracks().iter() {
            if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                track.stop();
            }
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}
