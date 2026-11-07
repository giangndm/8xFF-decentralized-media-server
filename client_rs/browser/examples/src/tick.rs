use crate::{
    harness::now_ms,
    media_evidence,
    session::{js_error, Session},
};
use atm0s_media_client_core::{ClientEvent, Command, Kind, MixerEvent, RoomEvent};
use wasm_bindgen::JsValue;

impl Session {
    pub async fn tick(&mut self) -> Result<(), JsValue> {
        self.runtime.pump_browser_events(now_ms());
        let mut events = self.runtime.drain().await?;
        events.extend(self.runtime.take_events());
        for event in events {
            self.handle_event(event).await?;
        }
        self.render_remote();
        self.advance_scenario().await.map_err(js_error)?;
        self.measure_media().await?;
        Ok(())
    }
    async fn handle_event(&mut self, event: ClientEvent) -> Result<(), JsValue> {
        self.log(&format!("{event:?}"));
        match event {
            ClientEvent::Room(RoomEvent::TrackStarted { source, kind, .. } | RoomEvent::TrackUpdated { source, kind, .. }) if source.peer != self.form.peer => {
                let current = if kind == Kind::Audio {
                    &mut self.remote_audio
                } else {
                    &mut self.remote_video
                };
                let changed = current.as_ref() != Some(&source);
                *current = Some(source.clone());
                if changed && !self.form.scenario.is_mixer() {
                    self.runtime
                        .command(Command::AttachReceiver {
                            name: if kind == Kind::Audio {
                                "audio_recv"
                            } else {
                                "video_recv"
                            }
                            .into(),
                            source,
                        })
                        .await
                        .map_err(js_error)?;
                }
            }
            ClientEvent::Message { peer, data, .. } if peer != self.form.peer && data == b"client-rs echo_fast" => self.message_seen = true,
            ClientEvent::Room(RoomEvent::TrackStopped { source, .. }) => {
                if self.remote_audio.as_ref() == Some(&source) {
                    self.remote_audio = None;
                }
                if self.remote_video.as_ref() == Some(&source) {
                    self.remote_video = None;
                }
            }
            ClientEvent::RoomChanged(None) => {
                self.remote_audio = None;
                self.remote_video = None;
            }
            ClientEvent::Mixer(MixerEvent::OutputChanged { slot, source: Some(source) }) if source.peer != self.form.peer => {
                self.mixer_slot = Some(slot);
                self.audio_baseline = None;
            }
            ClientEvent::Mixer(MixerEvent::OutputChanged { slot, source: None }) if self.mixer_slot == Some(slot) => {
                self.mixer_slot = None;
                self.audio_baseline = None;
            }
            ClientEvent::Error(error) => return Err(js_error(error)),
            _ => {}
        }
        Ok(())
    }
    async fn measure_media(&mut self) -> Result<(), JsValue> {
        if self.form.scenario.is_mixer() {
            if let Some(slot) = self.mixer_slot {
                let samples = media_evidence::decoded_audio_samples(&self.runtime, &format!("audio_{slot}")).await.map_err(js_error)?;
                match self.audio_baseline {
                    None => self.audio_baseline = Some(samples),
                    Some(baseline) if samples > baseline + 4_800.0 => {
                        if !self.media_reported {
                            self.report("media");
                            self.media_reported = true;
                        }
                        if self.operation_done {
                            self.finish_scenario();
                        }
                    }
                    _ => {}
                }
            }
        } else {
            let frames = self.video_frames("remote-video");
            if frames > 3 && !self.media_reported {
                self.report("media");
                self.media_reported = true;
            }
            if self.operation_done
                && self.video_baseline.is_some_and(|baseline| frames > baseline + 3)
                && (self.form.scenario != crate::scenarios::Scenario::MultiTrackReceivers || self.video_frames("extra-video") > 3)
            {
                self.finish_scenario();
            }
        }
        Ok(())
    }
    pub fn finish_scenario(&mut self) {
        if !self.scenario_reported {
            self.report("scenario");
            self.scenario_reported = true;
        }
    }
}
