//! Deterministic scripted adapter for the client core.

use atm0s_media_client_core::{
    Client, ClientError, DriverError, Effect, EffectCompletion, EffectEnvelope, EffectResult, HttpClient, HttpResponse, Input, IoDriver, LocalFuture, Output, Timestamp, WebRtcPeer,
};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct MockDriver {
    now: Timestamp,
    pub effects: Vec<Effect>,
    completions: VecDeque<Input>,
}

impl MockDriver {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn advance_time(&mut self, now: Timestamp) {
        self.now = now;
    }
    pub fn now(&self) -> Timestamp {
        self.now
    }
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
    pub fn complete(&mut self, input: Input) {
        self.completions.push_back(input);
    }
    pub fn pump(&mut self, client: &mut Client) {
        while let Some(input) = self.completions.pop_front() {
            client.on_input(self.now, input);
        }
    }
    pub fn handle_output(&mut self, output: Output) {
        if let Output::Effect(envelope) = output {
            self.effects.push(envelope.effect.clone());
            if matches!(envelope.effect, Effect::CreatePeer { .. }) {
                self.complete(Input::EffectCompleted(atm0s_media_client_core::EffectCompletion {
                    id: envelope.id,
                    generation: envelope.generation,
                    result: Ok(EffectResult::None),
                }));
            }
        }
    }
}

impl IoDriver for MockDriver {
    fn submit(&mut self, effect: EffectEnvelope) -> Result<(), DriverError> {
        self.effects.push(effect.effect.clone());
        if matches!(effect.effect, Effect::CreatePeer { .. }) {
            self.complete(Input::EffectCompleted(EffectCompletion {
                id: effect.id,
                generation: effect.generation,
                result: Ok(EffectResult::None),
            }));
        }
        Ok(())
    }
    fn poll_input(&mut self) -> Option<Input> {
        self.completions.pop_front()
    }
    fn schedule_wakeup(&mut self, _deadline: Option<Timestamp>) {}
    fn now(&self) -> Timestamp {
        self.now
    }
}

#[derive(Default)]
pub struct MockHttp {
    pub requests: Vec<atm0s_media_client_core::HttpRequest>,
    pub responses: VecDeque<Result<HttpResponse, ClientError>>,
}
impl HttpClient for MockHttp {
    fn request<'a>(&'a mut self, request: atm0s_media_client_core::HttpRequest) -> LocalFuture<'a, HttpResponse> {
        self.requests.push(request);
        let result = self.responses.pop_front().unwrap_or(Err(ClientError::Transport("no scripted HTTP response".into())));
        Box::pin(async move { result })
    }
}

#[derive(Default)]
pub struct MockPeer {
    pub effects: Vec<Effect>,
    pub results: VecDeque<Result<EffectResult, ClientError>>,
    pub inputs: VecDeque<Input>,
    pub tracks: HashMap<String, String>,
    pub streams: HashMap<String, String>,
    pub render_targets: HashMap<String, String>,
}
impl WebRtcPeer for MockPeer {
    type Track = String;
    type Stream = String;
    type RenderTarget = String;
    fn poll_input(&mut self) -> Option<Input> {
        self.inputs.pop_front()
    }
    fn register_track(&mut self, id: String, track: Self::Track) -> Result<(), ClientError> {
        self.tracks.insert(id, track);
        Ok(())
    }
    fn register_stream(&mut self, id: String, stream: Self::Stream) -> Result<(), ClientError> {
        self.streams.insert(id, stream);
        Ok(())
    }
    fn register_render_target(&mut self, id: String, target: Self::RenderTarget) -> Result<(), ClientError> {
        self.render_targets.insert(id, target);
        Ok(())
    }
    fn execute<'a>(&'a mut self, effect: Effect) -> LocalFuture<'a, EffectResult> {
        self.effects.push(effect);
        let result = self.results.pop_front().unwrap_or(Ok(EffectResult::None));
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atm0s_media_client_core::{Atm0sClient, ClientConfig, Command, Kind, SenderConfig};
    use prost::Message;
    #[test]
    fn records_connect_effect_and_scripted_time() {
        let mut c = Client::new(ClientConfig::default());
        c.command(
            0,
            Command::CreateSender {
                name: "a".into(),
                kind: Kind::Audio,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
        c.command(
            0,
            Command::Connect {
                gateway: "http://gateway".into(),
                token: "token".into(),
                offer: Some("v=0".into()),
                version: "mock".into(),
                join: None,
            },
        )
        .unwrap();
        let mut d = MockDriver::new();
        while let Some(out) = c.poll_output() {
            d.handle_output(out);
        }
        assert!(d.effects.iter().any(|e| matches!(e, Effect::Http(_))));
        d.advance_time(10_000);
        c.on_input(d.now(), Input::Tick);
        assert!(c.poll_output().is_some());
    }

    #[test]
    fn generic_facade_uses_distinct_http_and_peer_adapters() {
        let mut http = MockHttp::default();
        let response = media_server_protocol::protobuf::gateway::ConnectResponse {
            conn_id: "c".into(),
            sdp: "answer".into(),
            ice_lite: true,
        };
        http.responses.push_back(Ok(HttpResponse {
            status: 200,
            body: response.encode_to_vec(),
        }));
        let peer = MockPeer::default();
        let mut client = Atm0sClient::new(Client::new(ClientConfig::default()), http, peer);
        client.register_track("microphone", "track-handle".into()).unwrap();
        client.register_stream("camera", "stream-handle".into()).unwrap();
        client.register_render_target("preview", "element-handle".into()).unwrap();
        let operation = client
            .command(
                0,
                Command::Connect {
                    gateway: "http://gateway".into(),
                    token: "t".into(),
                    offer: Some("offer".into()),
                    version: "mock".into(),
                    join: None,
                },
            )
            .unwrap();
        futures::executor::block_on(client.pump()).unwrap();
        assert_eq!(client.http.requests.len(), 1);
        assert_eq!(client.peer.tracks.get("microphone").map(String::as_str), Some("track-handle"));
        assert_eq!(client.peer.streams.get("camera").map(String::as_str), Some("stream-handle"));
        assert_eq!(client.peer.render_targets.get("preview").map(String::as_str), Some("element-handle"));
        assert!(client.peer.effects.iter().any(|effect| matches!(effect, Effect::SetLocalDescription { .. })));
        assert_eq!(client.core.snapshot().session, atm0s_media_client_core::SessionState::Connecting);
        client.peer.inputs.push_back(Input::Rtc(atm0s_media_client_core::RtcEventEnvelope {
            generation: client.core.generation(),
            event: atm0s_media_client_core::RtcEvent::DataChannelOpen,
        }));
        futures::executor::block_on(client.pump()).unwrap();
        assert_eq!(client.core.snapshot().session, atm0s_media_client_core::SessionState::Connected);
        let completions: Vec<_> = std::iter::from_fn(|| client.poll_event())
            .filter_map(|event| match event {
                atm0s_media_client_core::ClientEvent::OperationCompleted { id, result } if id == operation => Some(result),
                _ => None,
            })
            .collect();
        assert!(matches!(completions.as_slice(), [Ok(atm0s_media_client_core::OperationResult::Connected)]));
    }
}
