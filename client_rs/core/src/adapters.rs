//! Generic adapter boundary for applications that want a facade rather than
//! manually driving the Sans-I/O [`Client`]. Futures are local (not `Send`) so
//! browser adapters can await JavaScript promises without thread requirements.

use crate::{Client, ClientError, ClientEvent, Effect, EffectCompletion, EffectEnvelope, EffectResult, Input, Output, Timestamp};
use media_server_protocol::protobuf::gateway;
use prost::Message;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + 'a>>;

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait HttpClient {
    fn request<'a>(&'a mut self, request: crate::HttpRequest) -> LocalFuture<'a, HttpResponse>;
}

pub trait WebRtcPeer {
    type Track;
    type Stream;
    type RenderTarget;
    fn register_track(&mut self, id: String, track: Self::Track) -> Result<(), ClientError>;
    fn register_stream(&mut self, id: String, stream: Self::Stream) -> Result<(), ClientError>;
    fn register_render_target(&mut self, id: String, target: Self::RenderTarget) -> Result<(), ClientError>;
    /// Return a callback event queued by the host (for example a data-channel
    /// message or an ICE candidate). The default keeps adapters that deliver
    /// callbacks by calling [`Atm0sClient::on_input`] directly lightweight.
    fn poll_input(&mut self) -> Option<Input> {
        None
    }
    fn execute<'a>(&'a mut self, effect: Effect) -> LocalFuture<'a, EffectResult>;
}

pub struct Atm0sClient<H, P> {
    pub core: Client,
    pub http: H,
    pub peer: P,
    events: VecDeque<ClientEvent>,
    now: Timestamp,
}

impl<H: HttpClient, P: WebRtcPeer> Atm0sClient<H, P> {
    pub fn new(core: Client, http: H, peer: P) -> Self {
        Self {
            core,
            http,
            peer,
            events: VecDeque::new(),
            now: 0,
        }
    }
    pub fn command(&mut self, now: Timestamp, command: crate::Command) -> Result<crate::OperationId, ClientError> {
        self.now = now;
        self.core.command(now, command)
    }
    /// Feed a browser/native callback into the Sans-I/O state machine.
    pub fn on_input(&mut self, now: Timestamp, input: Input) {
        self.now = now;
        self.core.on_input(now, input);
    }
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }
    pub fn register_track(&mut self, id: impl Into<String>, track: P::Track) -> Result<(), ClientError> {
        self.peer.register_track(id.into(), track)
    }
    pub fn register_stream(&mut self, id: impl Into<String>, stream: P::Stream) -> Result<(), ClientError> {
        self.peer.register_stream(id.into(), stream)
    }
    pub fn register_render_target(&mut self, id: impl Into<String>, target: P::RenderTarget) -> Result<(), ClientError> {
        self.peer.register_render_target(id.into(), target)
    }
    pub async fn pump(&mut self) -> Result<(), ClientError> {
        for _ in 0..1024 {
            let mut progressed = false;
            while let Some(input) = self.peer.poll_input() {
                progressed = true;
                self.core.on_input(self.now, input);
            }
            while let Some(input) = self.core.poll_output() {
                progressed = true;
                match input {
                    Output::Event(event) => self.events.push_back(event),
                    Output::Effect(effect) => self.execute(effect).await?,
                }
            }
            if !progressed {
                break;
            }
        }
        Ok(())
    }
    async fn execute(&mut self, envelope: EffectEnvelope) -> Result<(), ClientError> {
        let result = match envelope.effect {
            Effect::Http(request) => {
                let url = request.url.clone();
                match self.http.request(request).await {
                    Err(error) => Err(error),
                    Ok(response) if !(200..300).contains(&response.status) => Err(ClientError::Http {
                        status: response.status,
                        body: response.body,
                    }),
                    Ok(response) if url.ends_with("/connect") || url.contains("/restart-ice") => gateway::ConnectResponse::decode(response.body.as_slice())
                        .map(|result| EffectResult::Conn {
                            conn_id: result.conn_id,
                            sdp: result.sdp,
                            ice_lite: result.ice_lite,
                        })
                        .map_err(|error| ClientError::Protocol(error.to_string())),
                    Ok(response) => Ok(EffectResult::Bytes(response.body)),
                }
            }
            effect => self.peer.execute(effect).await,
        };
        self.core.on_input(
            self.now,
            Input::EffectCompleted(EffectCompletion {
                id: envelope.id,
                generation: envelope.generation,
                result,
            }),
        );
        Ok(())
    }
}
