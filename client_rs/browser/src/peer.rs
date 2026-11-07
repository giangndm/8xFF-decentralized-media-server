use crate::media::{BrowserStream, BrowserTrack};
use crate::render::unbind;
use atm0s_media_client_core::{ClientError, Effect, EffectResult, Input, WebRtcPeer};
use js_sys::Reflect;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    future::Future,
    pin::Pin,
    rc::Rc,
};
use wasm_bindgen::{closure::Closure, prelude::*, JsCast};
use web_sys::{HtmlMediaElement, RtcConfiguration, RtcRtpTransceiver};

mod events;
mod lowlevel;
pub use lowlevel::BrowserPeer;

#[derive(Debug, Clone)]
pub enum DataEvent {
    Open,
    Close,
    Error(String),
    Message(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum PeerEvent {
    IceCandidate(String),
    Data(DataEvent),
    Track { track: BrowserTrack, streams: Vec<BrowserStream> },
    ConnectionState(String),
    IceConnectionState(String),
}

pub type EventSink = js_sys::Function;

/// Typed WebRTC adapter facade. The core sees opaque operations while browser
/// applications can still access the concrete peer for rendering/capture.
pub struct BrowserWebRtcPeer {
    peer: BrowserPeer,
    config: Option<RtcConfiguration>,
    transceivers: HashMap<String, RtcRtpTransceiver>,
    tracks: HashMap<String, BrowserTrack>,
    streams: HashMap<String, BrowserStream>,
    targets: HashMap<String, web_sys::HtmlMediaElement>,
    input_events: Rc<RefCell<VecDeque<(u64, JsValue)>>>,
    callback_generation: Rc<Cell<u64>>,
    internal_sink: Option<Closure<dyn FnMut(JsValue)>>,
    external_sink: Option<EventSink>,
}

impl BrowserWebRtcPeer {
    pub fn new(config: Option<RtcConfiguration>) -> Result<Self, JsValue> {
        let mut peer = BrowserPeer::new(config.clone())?;
        let callback_generation = Rc::new(Cell::new(0));
        let event_generation = callback_generation.clone();
        let input_events = Rc::new(RefCell::new(VecDeque::new()));
        let queue = input_events.clone();
        let internal_sink = Closure::wrap(Box::new(move |event: JsValue| {
            queue.borrow_mut().push_back((event_generation.get(), event));
        }) as Box<dyn FnMut(JsValue)>);
        peer.install_event_sink(internal_sink.as_ref().unchecked_ref::<js_sys::Function>().clone())?;
        Ok(Self {
            peer,
            config,
            transceivers: HashMap::new(),
            tracks: HashMap::new(),
            streams: HashMap::new(),
            targets: HashMap::new(),
            input_events,
            callback_generation,
            internal_sink: Some(internal_sink),
            external_sink: None,
        })
    }
    pub fn peer(&self) -> &BrowserPeer {
        &self.peer
    }
    pub fn peer_mut(&mut self) -> &mut BrowserPeer {
        &mut self.peer
    }
    pub fn control_open(&self) -> bool {
        self.peer.control_open()
    }
    pub async fn create_offer(&self) -> Result<String, JsValue> {
        self.peer.create_offer().await
    }
    pub async fn restart_ice_offer(&self) -> Result<String, JsValue> {
        self.peer.restart_ice_offer().await
    }
    pub async fn set_local_description(&self, sdp: &str) -> Result<(), JsValue> {
        self.peer.set_local_description(sdp).await
    }
    /// Install the low-level callback used to bridge browser WebRTC events
    /// into `Client::on_input` (or another event queue owned by the driver).
    pub fn install_event_sink(&mut self, sink: EventSink) -> Result<(), JsValue> {
        self.peer.install_event_sink(sink.clone())?;
        self.external_sink = Some(sink);
        self.internal_sink = None;
        Ok(())
    }
    pub fn transceiver(&self, name: &str) -> Option<&RtcRtpTransceiver> {
        self.transceivers.get(name)
    }
    pub fn transceiver_name(&self, actual: &RtcRtpTransceiver) -> Option<String> {
        self.transceivers.iter().find_map(|(name, known)| {
            let known_value: &JsValue = known.as_ref();
            let actual_value: &JsValue = actual.as_ref();
            (known_value == actual_value).then(|| name.clone())
        })
    }
    pub fn track(&self, id: &str) -> Option<&BrowserTrack> {
        self.tracks.get(id)
    }
    pub fn stream(&self, id: &str) -> Option<&BrowserStream> {
        self.streams.get(id)
    }
    pub fn prepare_transceiver(&mut self, name: impl Into<String>, kind: atm0s_media_client_core::Kind, direction: atm0s_media_client_core::Direction, simulcast: bool) -> Result<(), JsValue> {
        let name = name.into();
        if self.transceivers.contains_key(&name) {
            return Ok(());
        }
        let media_kind = if kind == atm0s_media_client_core::Kind::Audio {
            "audio"
        } else {
            "video"
        };
        let rtc_direction = if direction == atm0s_media_client_core::Direction::SendOnly {
            web_sys::RtcRtpTransceiverDirection::Sendonly
        } else {
            web_sys::RtcRtpTransceiverDirection::Recvonly
        };
        self.transceivers.insert(name, self.peer.add_transceiver(media_kind, rtc_direction, simulcast)?);
        Ok(())
    }
    /// Register a render target without requiring a global DOM id.
    pub fn register_render_target(&mut self, id: impl Into<String>, target: web_sys::HtmlMediaElement) {
        self.targets.insert(id.into(), target);
    }
}

fn js_error(context: &str, error: JsValue) -> ClientError {
    ClientError::Transport(format!("{context}: {error:?}"))
}

impl WebRtcPeer for BrowserWebRtcPeer {
    type Track = BrowserTrack;
    type Stream = BrowserStream;
    type RenderTarget = HtmlMediaElement;

    fn register_track(&mut self, id: String, track: Self::Track) -> Result<(), ClientError> {
        self.tracks.insert(id, track);
        Ok(())
    }

    fn register_stream(&mut self, id: String, stream: Self::Stream) -> Result<(), ClientError> {
        self.streams.insert(id, stream);
        Ok(())
    }

    fn register_render_target(&mut self, id: String, target: Self::RenderTarget) -> Result<(), ClientError> {
        self.targets.insert(id, target);
        Ok(())
    }

    fn poll_input(&mut self) -> Option<Input> {
        self.next_input()
    }

    fn execute<'a>(&'a mut self, effect: Effect) -> Pin<Box<dyn Future<Output = Result<EffectResult, ClientError>> + 'a>> {
        let generation = match &effect {
            Effect::CreatePeer { generation }
            | Effect::CreateOffer { generation, .. }
            | Effect::SetLocalDescription { generation, .. }
            | Effect::SetRemoteDescription { generation, .. }
            | Effect::AddTransceiver { generation, .. }
            | Effect::ReplaceTrack { generation, .. }
            | Effect::SendData { generation, .. }
            | Effect::ClosePeer { generation } => Some(*generation),
            Effect::Http(_) | Effect::BindRender { .. } | Effect::UnbindRender { .. } => None,
        };
        if let Some(generation) = generation {
            self.callback_generation.set(generation);
        }
        Box::pin(async move {
            match effect {
                Effect::CreatePeer { .. } => {
                    let state = Reflect::get(self.peer.raw(), &"connectionState".into()).ok().and_then(|state| state.as_string());
                    if matches!(state.as_deref(), Some("closed" | "failed")) {
                        let mut peer = BrowserPeer::new(self.config.clone()).map_err(|e| js_error("recreate peer", e))?;
                        let sink = self
                            .external_sink
                            .clone()
                            .or_else(|| self.internal_sink.as_ref().map(|sink| sink.as_ref().unchecked_ref::<js_sys::Function>().clone()));
                        if let Some(sink) = sink {
                            peer.install_event_sink(sink).map_err(|e| js_error("install callbacks", e))?;
                        }
                        self.peer = peer;
                        for name in self.transceivers.keys() {
                            self.streams.remove(name);
                        }
                        self.transceivers.clear();
                        self.input_events.borrow_mut().clear();
                    }
                    Ok(EffectResult::None)
                }
                Effect::CreateOffer { restart, .. } => {
                    let sdp = if restart {
                        self.peer.restart_ice_offer().await
                    } else {
                        self.peer.create_offer().await
                    };
                    sdp.map(EffectResult::Sdp).map_err(|e| js_error("create offer", e))
                }
                Effect::SetLocalDescription { sdp, .. } => self
                    .peer
                    .set_local_description(&sdp)
                    .await
                    .map(|_| EffectResult::None)
                    .map_err(|e| js_error("set local description", e)),
                Effect::SetRemoteDescription { sdp, .. } => self.peer.set_remote_answer(&sdp).await.map(|_| EffectResult::None).map_err(|e| js_error("set remote description", e)),
                Effect::AddTransceiver { name, kind, direction, simulcast, .. } => {
                    self.prepare_transceiver(name, kind, direction, simulcast).map_err(|e| js_error("add transceiver", e))?;
                    Ok(EffectResult::None)
                }
                Effect::ReplaceTrack { name, source_id, .. } => {
                    let transceiver = self.transceivers.get(&name).cloned().ok_or_else(|| ClientError::InvalidInput(format!("unknown transceiver {name}")))?;
                    let track = match source_id.as_deref() {
                        Some(id) => Some(self.tracks.get(id).ok_or_else(|| ClientError::InvalidInput(format!("unknown track {id}")))?.0.clone()),
                        None => None,
                    };
                    self.peer
                        .replace_track(&transceiver, track.as_ref())
                        .await
                        .map(|_| EffectResult::None)
                        .map_err(|e| js_error("replace track", e))
                }
                Effect::SendData { data, .. } => self.peer.send_control(&data).map(|_| EffectResult::None).map_err(|e| js_error("send data", e)),
                Effect::ClosePeer { .. } => {
                    self.peer.close();
                    Ok(EffectResult::None)
                }
                Effect::BindRender { stream, target } => {
                    let element = if let Some(element) = self.targets.get(&target) {
                        element.clone()
                    } else {
                        let document = web_sys::window()
                            .and_then(|window| window.document())
                            .ok_or_else(|| ClientError::Transport("document unavailable".into()))?;
                        document
                            .get_element_by_id(&target)
                            .ok_or_else(|| ClientError::InvalidInput(format!("unknown render target {target}")))?
                            .dyn_into::<web_sys::HtmlMediaElement>()
                            .map_err(|_| ClientError::InvalidInput(format!("target {target} is not a media element")))?
                    };
                    let stream = self.streams.get(&stream).cloned().ok_or_else(|| ClientError::InvalidInput(format!("unknown stream {stream}")))?;
                    crate::render::bind_and_play(&element, &stream.0)
                        .await
                        .map(|_| EffectResult::None)
                        .map_err(|e| js_error("bind render", e))
                }
                Effect::UnbindRender { target, .. } => {
                    let element = self.targets.get(&target).cloned().or_else(|| {
                        web_sys::window()
                            .and_then(|window| window.document())
                            .and_then(|document| document.get_element_by_id(&target))
                            .and_then(|element| element.dyn_into::<web_sys::HtmlMediaElement>().ok())
                    });
                    if let Some(element) = element {
                        unbind(&element);
                    }
                    Ok(EffectResult::None)
                }
                Effect::Http(_) => Err(ClientError::InvalidInput("HTTP effects belong to HttpClient".into())),
            }
        })
    }
}
