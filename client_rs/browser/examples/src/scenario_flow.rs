use crate::{
    capture::Capture,
    harness::now_ms,
    scenarios::{self, Scenario},
    session::Session,
};
use atm0s_media_client_core::{ClientError, Command, Kind};

impl Session {
    pub async fn advance_scenario(&mut self) -> Result<(), ClientError> {
        if self.form.scenario == Scenario::EchoFast {
            if !self.channel_ready {
                scenarios::echo_fast::setup(&mut self.runtime).await?;
                self.channel_ready = true;
            }
            if now_ms().saturating_sub(self.last_publish_ms) >= 500 {
                self.runtime
                    .command(Command::PublishMessage {
                        label: "testing".into(),
                        data: b"client-rs echo_fast".to_vec(),
                    })
                    .await?;
                self.last_publish_ms = now_ms();
            }
            if self.message_seen {
                self.finish_scenario();
            }
            return Ok(());
        }
        if self.form.scenario.is_mixer() {
            return self.advance_mixer().await;
        }
        if self.operation_done || self.video_frames("remote-video") < 4 {
            return Ok(());
        }
        match self.form.scenario {
            Scenario::EchoSimulcast => scenarios::echo_simulcast::change_layers(&mut self.runtime).await?,
            Scenario::SwitchSenderTrack => self.replacement = Some(Capture(scenarios::switch_sender_track::switch(&mut self.runtime).await?)),
            Scenario::RestartIce => scenarios::echo_restart_ice::restart(&mut self.runtime).await?,
            Scenario::EchoLazy => {
                let join = self.join_config();
                scenarios::echo_lazy::cycle(&mut self.runtime, join, self.form.token.clone()).await?;
                self.remote_audio = None;
                self.remote_video = None;
            }
            Scenario::MultiTrackReceivers => {
                let Some(source) = self.remote_video.clone() else { return Ok(()) };
                scenarios::multi_track_receivers::attach(&mut self.runtime, "video_recv_extra".into(), Kind::Video, source).await?;
                if let Some(element) = self.document.get_element_by_id("extra-video") {
                    let _ = element.remove_attribute("hidden");
                }
            }
            _ => {}
        }
        self.operation_done = true;
        self.video_baseline = Some(self.video_frames("remote-video"));
        Ok(())
    }

    async fn advance_mixer(&mut self) -> Result<(), ClientError> {
        if self.form.scenario == Scenario::AudioMixerAuto {
            // AUTO selects room sources through the join feature; no manual attach.
            self.operation_done = self.mixer_slot.is_some();
        } else if let Some(source) = self.remote_audio.clone() {
            if !self.mixer_attached {
                scenarios::audio_mixer_manual::attach(&mut self.runtime, source).await?;
                self.mixer_attached = true;
            } else if self.media_reported && !self.operation_done {
                scenarios::audio_mixer_manual::cycle(&mut self.runtime, source).await?;
                self.operation_done = true;
                self.audio_baseline = None;
            }
        }
        Ok(())
    }
}
