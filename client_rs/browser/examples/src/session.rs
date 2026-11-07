use crate::{
    capture::Capture,
    harness::{append_log, set_assert},
    scenarios::{self, Scenario},
};
use atm0s_media_client_browser::{get_user_media, BrowserRuntime};
use atm0s_media_client_core::{ClientConfig, Command, JoinConfig, Kind, ReceiverConfig, SenderConfig, Source};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Document;

pub struct ConnectForm {
    pub gateway: String,
    pub room: String,
    pub peer: String,
    pub token: String,
    pub scenario: Scenario,
}

pub struct Session {
    pub runtime: BrowserRuntime,
    pub local: Capture,
    pub replacement: Option<Capture>,
    pub document: Document,
    pub form: ConnectForm,
    pub remote_audio: Option<Source>,
    pub remote_video: Option<Source>,
    pub message_seen: bool,
    pub channel_ready: bool,
    pub last_publish_ms: u64,
    pub operation_done: bool,
    pub video_baseline: Option<u32>,
    pub audio_baseline: Option<f64>,
    pub mixer_slot: Option<u32>,
    pub mixer_attached: bool,
    pub scenario_reported: bool,
    pub media_reported: bool,
}

impl Session {
    pub async fn connect(document: Document, form: ConnectForm) -> Result<Self, JsValue> {
        let runtime = BrowserRuntime::new(ClientConfig::default(), None)?;
        let local = Capture(get_user_media(true, !form.scenario.is_mixer()).await?);
        let mut session = Self {
            runtime,
            local,
            replacement: None,
            document,
            form,
            remote_audio: None,
            remote_video: None,
            message_seen: false,
            channel_ready: false,
            last_publish_ms: 0,
            operation_done: false,
            video_baseline: None,
            audio_baseline: None,
            mixer_slot: None,
            mixer_attached: false,
            scenario_reported: false,
            media_reported: false,
        };
        let lazy = session.form.scenario == Scenario::EchoLazy;
        if !lazy {
            session.configure_tracks().await?;
        }
        let join = if lazy {
            None
        } else {
            Some(session.join_config())
        };
        session
            .runtime
            .command(Command::Connect {
                gateway: session.form.gateway.clone(),
                token: session.form.token.clone(),
                offer: None,
                version: "client-rs-browser@0.1".into(),
                join,
            })
            .await
            .map_err(js_error)?;
        session.report("control");
        if lazy {
            session.configure_tracks().await?;
            session
                .runtime
                .command(Command::Join {
                    info: session.join_config(),
                    token: session.form.token.clone(),
                })
                .await
                .map_err(js_error)?;
        }
        session.bind_stream("local", "local-video");
        Ok(session)
    }

    async fn configure_tracks(&mut self) -> Result<(), JsValue> {
        self.runtime.register_stream("local", self.local.0 .0.clone());
        for (kind, tracks, name) in [
            (Kind::Audio, self.local.0 .0.get_audio_tracks(), "audio_main"),
            (Kind::Video, self.local.0 .0.get_video_tracks(), "video_main"),
        ] {
            if tracks.length() == 0 {
                continue;
            }
            let track = tracks.get(0).dyn_into::<web_sys::MediaStreamTrack>()?;
            self.runtime.register_track(track.id(), track.clone());
            self.runtime
                .command(Command::CreateSender {
                    name: name.into(),
                    kind,
                    config: SenderConfig {
                        simulcast: kind == Kind::Video && self.form.scenario == Scenario::EchoSimulcast,
                        ..Default::default()
                    },
                })
                .await
                .map_err(js_error)?;
            self.runtime
                .command(Command::AttachSender {
                    name: name.into(),
                    source_id: track.id(),
                })
                .await
                .map_err(js_error)?;
        }
        let receivers: Vec<_> = if self.form.scenario.is_mixer() {
            (0..3).map(|slot| (format!("audio_{slot}"), Kind::Audio)).collect()
        } else {
            vec![("audio_recv".into(), Kind::Audio), ("video_recv".into(), Kind::Video)]
        };
        for (name, kind) in receivers {
            self.runtime
                .command(Command::CreateReceiver {
                    name,
                    kind,
                    config: ReceiverConfig::default(),
                })
                .await
                .map_err(js_error)?;
        }
        Ok(())
    }

    pub fn join_config(&self) -> JoinConfig {
        JoinConfig {
            room: self.form.room.clone(),
            peer: self.form.peer.clone(),
            metadata: None,
            publish_peer: true,
            publish_tracks: true,
            subscribe_peers: true,
            subscribe_tracks: true,
            mixer: match self.form.scenario {
                Scenario::AudioMixerAuto => Some(scenarios::audio_mixer_auto::config()),
                Scenario::AudioMixerManual => Some(scenarios::audio_mixer_manual::config()),
                _ => None,
            },
        }
    }
    pub fn log(&self, message: &str) {
        if let Some(log) = self.document.get_element_by_id("event-log") {
            append_log(&log, message);
        }
    }
    pub fn report(&self, name: &str) {
        set_assert(&self.document, name, "pass");
        self.log(&format!("{name}:pass"));
    }
    pub async fn disconnect(&mut self) -> Result<(), JsValue> {
        self.runtime.command(Command::Disconnect).await.map_err(js_error)?;
        self.runtime.drain().await?;
        self.clear_media();
        self.local.stop();
        if let Some(replacement) = &self.replacement {
            replacement.stop();
        }
        let closed = js_sys::Reflect::get(self.runtime.core.peer.peer().raw().as_ref(), &"connectionState".into())?.as_string().as_deref() == Some("closed");
        if !closed || !self.local.stopped() || self.replacement.as_ref().is_some_and(|capture| !capture.stopped()) {
            return Err(JsValue::from_str("peer or owned capture did not close"));
        }
        self.report("cleanup");
        Ok(())
    }
}

pub fn js_error(error: atm0s_media_client_core::ClientError) -> JsValue {
    JsValue::from_str(&format!("client: {error:?}"))
}
