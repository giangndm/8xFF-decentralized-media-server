//! Public data model shared by the Sans-I/O state machine and adapters.

pub type Timestamp = u64;
pub type OperationId = u64;
pub type EffectId = u64;
pub type Generation = u64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrackId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    InvalidState(&'static str),
    InvalidInput(String),
    Transport(String),
    Http { status: u16, body: Vec<u8> },
    Protocol(String),
    Timeout { operation: OperationId },
    QueueFull,
    StaleGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    New,
    Connecting,
    Connected,
    Reconnecting,
    Closing,
    Closed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Video,
}
impl From<Kind> for i32 {
    fn from(k: Kind) -> i32 {
        match k {
            Kind::Audio => 0,
            Kind::Video => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SenderConfig {
    pub priority: u32,
    pub bitrate: i32,
    pub simulcast: bool,
}
impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            priority: 1,
            bitrate: 0,
            simulcast: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiverConfig {
    pub priority: u32,
    pub max_spatial: u32,
    pub max_temporal: u32,
    pub min_spatial: Option<u32>,
    pub min_temporal: Option<u32>,
}
impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            priority: 1,
            max_spatial: 2,
            max_temporal: 2,
            min_spatial: None,
            min_temporal: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SenderState {
    pub name: String,
    pub kind: Kind,
    pub source: Option<String>,
    pub source_metadata: Option<String>,
    pub source_screen: bool,
    pub config: SenderConfig,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiverState {
    pub name: String,
    pub kind: Kind,
    pub source: Option<Source>,
    pub config: ReceiverConfig,
    pub ready: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub peer: String,
    pub track: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomInfo {
    pub room: String,
    pub peer: String,
    pub metadata: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub rpc_timeout_ms: Timestamp,
    pub max_message_size: usize,
    pub max_pending: usize,
    /// Admission high-water mark. Already accepted completions and callback
    /// events remain lossless and may exceed this mark; drivers must drain them.
    pub max_outputs: usize,
}
impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            rpc_timeout_ms: 5_000,
            max_message_size: 1024 * 1024,
            max_pending: 256,
            max_outputs: 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    CreateSender {
        name: String,
        kind: Kind,
        config: SenderConfig,
    },
    CreateReceiver {
        name: String,
        kind: Kind,
        config: ReceiverConfig,
    },
    AttachSender {
        name: String,
        source_id: String,
    },
    AttachSenderWithMetadata {
        name: String,
        source_id: String,
        metadata: Option<String>,
        screen: bool,
    },
    DetachSender {
        name: String,
    },
    ConfigureSender {
        name: String,
        config: SenderConfig,
    },
    AttachReceiver {
        name: String,
        source: Source,
    },
    DetachReceiver {
        name: String,
    },
    ConfigureReceiver {
        name: String,
        config: ReceiverConfig,
    },
    Connect {
        gateway: String,
        token: String,
        /// Supply an existing offer, or let the core prepare the peer and generate one.
        offer: Option<String>,
        version: String,
        join: Option<JoinConfig>,
    },
    RestartIce {
        /// Supply a restart offer, or generate it through the WebRTC adapter.
        offer: Option<String>,
    },
    Join {
        info: JoinConfig,
        token: String,
    },
    Leave,
    CreateMessageChannel {
        label: String,
        publish: bool,
    },
    CloseMessageChannel {
        label: String,
    },
    SetMessagePublishing {
        label: String,
        enabled: bool,
    },
    PublishMessage {
        label: String,
        data: Vec<u8>,
    },
    MixerAttach {
        sources: Vec<Source>,
    },
    MixerDetach {
        sources: Vec<Source>,
    },
    Disconnect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinConfig {
    pub room: String,
    pub peer: String,
    pub metadata: Option<String>,
    pub publish_peer: bool,
    pub publish_tracks: bool,
    pub subscribe_peers: bool,
    pub subscribe_tracks: bool,
    pub mixer: Option<MixerConfig>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixerConfig {
    pub mode: i32,
    pub outputs: u32,
    pub sources: Vec<Source>,
}

#[derive(Clone, Debug)]
pub enum HttpMethod {
    Post,
}
#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum Effect {
    Http(HttpRequest),
    CreatePeer {
        generation: Generation,
    },
    CreateOffer {
        generation: Generation,
        restart: bool,
    },
    SetLocalDescription {
        generation: Generation,
        sdp: String,
    },
    SetRemoteDescription {
        generation: Generation,
        sdp: String,
    },
    AddTransceiver {
        generation: Generation,
        name: String,
        kind: Kind,
        direction: Direction,
        simulcast: bool,
    },
    ReplaceTrack {
        generation: Generation,
        name: String,
        source_id: Option<String>,
    },
    SendData {
        generation: Generation,
        data: Vec<u8>,
    },
    ClosePeer {
        generation: Generation,
    },
    BindRender {
        stream: String,
        target: String,
    },
    UnbindRender {
        stream: String,
        target: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    SendOnly,
    RecvOnly,
}

#[derive(Clone, Debug)]
pub struct EffectEnvelope {
    pub id: EffectId,
    pub generation: Generation,
    pub operation: Option<OperationId>,
    pub effect: Effect,
}

#[derive(Clone, Debug)]
pub enum EffectResult {
    None,
    Bytes(Vec<u8>),
    Sdp(String),
    Conn { conn_id: String, sdp: String, ice_lite: bool },
    DataChannelOpen,
    TrackReady { name: String, track_id: String },
}
#[derive(Clone, Debug)]
pub struct EffectCompletion {
    pub id: EffectId,
    pub generation: Generation,
    pub result: Result<EffectResult, ClientError>,
}

#[derive(Clone, Debug)]
pub enum RtcEvent {
    DataChannelOpen,
    Data(Vec<u8>),
    LocalCandidate(String),
    ConnectionState(String),
    TrackReady { name: String, track_id: String },
    NegotiationNeeded,
}
#[derive(Clone, Debug)]
pub struct RtcEventEnvelope {
    pub generation: Generation,
    pub event: RtcEvent,
}

#[derive(Clone, Debug)]
pub enum ClientEvent {
    SessionChanged(SessionState),
    RoomChanged(Option<RoomInfo>),
    Room(RoomEvent),
    SenderStatus { name: String, active: bool },
    ReceiverStatus { name: String, active: bool },
    ReceiverReady { name: String, track_id: String },
    Message { label: String, peer: String, data: Vec<u8> },
    Mixer(MixerEvent),
    OperationCompleted { id: OperationId, result: Result<OperationResult, ClientError> },
    Error(ClientError),
}
#[derive(Clone, Debug)]
pub enum RoomEvent {
    PeerJoined { peer: String, metadata: Option<String> },
    PeerUpdated { peer: String, metadata: Option<String> },
    PeerLeft { peer: String },
    TrackStarted { source: Source, kind: Kind, metadata: Option<String> },
    TrackUpdated { source: Source, kind: Kind, metadata: Option<String> },
    TrackStopped { source: Source, kind: Kind },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MixerEvent {
    OutputChanged { slot: u32, source: Option<Source> },
    VoiceActivity { source: Source, active: bool, level: Option<i32> },
}
#[derive(Clone, Debug)]
pub enum OperationResult {
    Connected,
    Disconnected,
    Joined,
    Left,
    Updated,
    Published,
    Created(String),
}
#[derive(Clone, Debug)]
pub enum Output {
    Effect(EffectEnvelope),
    Event(ClientEvent),
}
#[derive(Clone, Debug)]
pub enum Input {
    EffectCompleted(EffectCompletion),
    Rtc(RtcEventEnvelope),
    Tick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverError(pub String);

/// Optional synchronous driver boundary for host/native adapters and tests.
/// Browser adapters may enqueue effects and resolve them from JavaScript
/// promises; they can use the same Effect/Input types without implementing
/// this trait.
pub trait IoDriver {
    fn submit(&mut self, effect: EffectEnvelope) -> Result<(), DriverError>;
    fn poll_input(&mut self) -> Option<Input>;
    fn schedule_wakeup(&mut self, deadline: Option<Timestamp>);
    fn now(&self) -> Timestamp;
}

#[derive(Clone, Debug)]
pub struct ClientSnapshot {
    pub session: SessionState,
    pub generation: Generation,
    pub conn_id: Option<String>,
    pub ice_lite: bool,
    pub room: Option<RoomInfo>,
    pub senders: Vec<SenderState>,
    pub receivers: Vec<ReceiverState>,
    pub channels: Vec<String>,
}
