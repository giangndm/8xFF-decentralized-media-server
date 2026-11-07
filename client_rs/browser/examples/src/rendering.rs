use crate::session::Session;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{HtmlMediaElement, HtmlVideoElement};

impl Session {
    pub fn bind_stream(&self, stream_name: &str, element_id: &str) {
        let Some(stream) = self.runtime.stream(stream_name) else { return };
        let Some(element) = self.document.get_element_by_id(element_id).and_then(|element| element.dyn_into::<HtmlMediaElement>().ok()) else {
            return;
        };
        if element.src_object().is_some_and(|current| current.id() == stream.0.id()) {
            return;
        }
        element.set_src_object(Some(&stream.0));
        self.log(&format!("render:{stream_name}->{element_id}"));
        if let Ok(promise) = element.play() {
            spawn_local(async move {
                if let Err(error) = JsFuture::from(promise).await {
                    web_sys::console::warn_1(&error);
                }
            });
        }
    }
    pub fn video_frames(&self, id: &str) -> u32 {
        self.document
            .get_element_by_id(id)
            .and_then(|element| element.dyn_into::<HtmlVideoElement>().ok())
            .map(|video| video.get_video_playback_quality().total_video_frames())
            .unwrap_or(0)
    }
    pub fn render_remote(&self) {
        self.bind_stream("video_recv", "remote-video");
        self.bind_stream("audio_recv", "remote-audio");
        self.bind_stream("video_recv_extra", "extra-video");
        if let Some(slot) = self.mixer_slot {
            self.bind_stream(&format!("audio_{slot}"), "remote-audio");
        }
    }
    pub fn clear_media(&self) {
        for id in ["local-video", "remote-video", "remote-audio", "extra-video"] {
            if let Some(element) = self.document.get_element_by_id(id).and_then(|element| element.dyn_into::<HtmlMediaElement>().ok()) {
                let _ = element.pause();
                element.set_src_object(None);
            }
        }
    }
}
