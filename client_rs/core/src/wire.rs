use crate::*;
use media_server_protocol::protobuf::{features, gateway, session, shared};

#[derive(Clone, Copy)]
pub(crate) enum ResponseKind {
    Session,
    Sender,
    Receiver,
    Message,
    Mixer,
}
pub(crate) fn response_matches(kind: ResponseKind, response: &session::response::Response) -> bool {
    matches!(
        (kind, response),
        (ResponseKind::Session, session::response::Response::Session(_))
            | (ResponseKind::Sender, session::response::Response::Sender(_))
            | (ResponseKind::Receiver, session::response::Response::Receiver(_))
            | (ResponseKind::Message, session::response::Response::MessageChannel(_))
            | (ResponseKind::Mixer, session::response::Response::Features(_))
    )
}
fn kind_proto(k: Kind) -> i32 {
    k.into()
}
fn sender_proto(s: &SenderState) -> shared::Sender {
    shared::Sender {
        kind: kind_proto(s.kind),
        name: s.name.clone(),
        state: Some(shared::sender::State {
            config: Some(shared::sender::Config {
                priority: s.config.priority,
                bitrate: s.config.bitrate,
            }),
            source: s.source.as_ref().map(|id| shared::sender::Source {
                id: id.clone(),
                screen: s.source_screen,
                metadata: s.source_metadata.clone(),
            }),
        }),
    }
}
fn receiver_proto(r: &ReceiverState) -> shared::Receiver {
    shared::Receiver {
        kind: kind_proto(r.kind),
        name: r.name.clone(),
        state: Some(shared::receiver::State {
            config: Some(shared::receiver::Config {
                priority: r.config.priority,
                max_spatial: r.config.max_spatial,
                max_temporal: r.config.max_temporal,
                min_spatial: r.config.min_spatial,
                min_temporal: r.config.min_temporal,
            }),
            source: r.source.as_ref().map(|s| shared::receiver::Source {
                peer: s.peer.clone(),
                track: s.track.clone(),
            }),
        }),
    }
}
pub(crate) fn join_proto(j: &JoinConfig) -> session::RoomJoin {
    session::RoomJoin {
        room: j.room.clone(),
        peer: j.peer.clone(),
        publish: Some(shared::RoomInfoPublish {
            peer: j.publish_peer,
            tracks: j.publish_tracks,
        }),
        subscribe: Some(shared::RoomInfoSubscribe {
            peers: j.subscribe_peers,
            tracks: j.subscribe_tracks,
        }),
        features: j.mixer.as_ref().map(|m| features::Config {
            mixer: Some(features::mixer::Config {
                mode: m.mode,
                outputs: (0..m.outputs).map(|i| format!("audio_{i}")).collect(),
                sources: m
                    .sources
                    .iter()
                    .map(|s| shared::receiver::Source {
                        peer: s.peer.clone(),
                        track: s.track.clone(),
                    })
                    .collect(),
            }),
        }),
        metadata: j.metadata.clone(),
    }
}
pub(crate) fn gateway_request(version: &str, join: Option<&JoinConfig>, senders: &[SenderState], receivers: &[ReceiverState], sdp: &str) -> gateway::ConnectRequest {
    gateway::ConnectRequest {
        version: version.into(),
        join: join.map(join_proto),
        tracks: Some(shared::Tracks {
            receivers: receivers.iter().map(receiver_proto).collect(),
            senders: senders.iter().map(sender_proto).collect(),
        }),
        sdp: sdp.into(),
    }
}
pub(crate) fn update_sdp_request(senders: &[SenderState], receivers: &[ReceiverState], sdp: &str) -> session::Request {
    session_request(session::request::session::Request::Sdp(session::request::session::UpdateSdp {
        tracks: Some(shared::Tracks {
            receivers: receivers.iter().map(receiver_proto).collect(),
            senders: senders.iter().map(sender_proto).collect(),
        }),
        sdp: sdp.into(),
    }))
}
pub(crate) fn session_request(r: session::request::session::Request) -> session::Request {
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::Session(session::request::Session { request: Some(r) })),
    }
}
pub(crate) fn receiver_request(r: &ReceiverState, attach: bool) -> session::Request {
    let req = if attach {
        session::request::receiver::Request::Attach(session::request::receiver::Attach {
            source: r.source.as_ref().map(|s| shared::receiver::Source {
                peer: s.peer.clone(),
                track: s.track.clone(),
            }),
            config: Some(shared::receiver::Config {
                priority: r.config.priority,
                max_spatial: r.config.max_spatial,
                max_temporal: r.config.max_temporal,
                min_spatial: r.config.min_spatial,
                min_temporal: r.config.min_temporal,
            }),
        })
    } else {
        session::request::receiver::Request::Detach(session::request::receiver::Detach {})
    };
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::Receiver(session::request::Receiver {
            name: r.name.clone(),
            request: Some(req),
        })),
    }
}

pub(crate) fn receiver_config_request(r: &ReceiverState) -> session::Request {
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::Receiver(session::request::Receiver {
            name: r.name.clone(),
            request: Some(session::request::receiver::Request::Config(shared::receiver::Config {
                priority: r.config.priority,
                max_spatial: r.config.max_spatial,
                max_temporal: r.config.max_temporal,
                min_spatial: r.config.min_spatial,
                min_temporal: r.config.min_temporal,
            })),
        })),
    }
}
pub(crate) enum SenderAction {
    Attach,
    Detach,
    Config,
}
pub(crate) fn sender_request(s: &SenderState, action: SenderAction) -> session::Request {
    let request = match action {
        SenderAction::Attach => session::request::sender::Request::Attach(session::request::sender::Attach {
            source: s.source.as_ref().map(|id| shared::sender::Source {
                id: id.clone(),
                screen: s.source_screen,
                metadata: s.source_metadata.clone(),
            }),
            config: Some(shared::sender::Config {
                priority: s.config.priority,
                bitrate: s.config.bitrate,
            }),
        }),
        SenderAction::Detach => session::request::sender::Request::Detach(session::request::sender::Detach {}),
        SenderAction::Config => session::request::sender::Request::Config(shared::sender::Config {
            priority: s.config.priority,
            bitrate: s.config.bitrate,
        }),
    };
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::Sender(session::request::Sender {
            name: s.name.clone(),
            request: Some(request),
        })),
    }
}
pub(crate) enum MessageAction {
    StartPub,
    StopPub,
    Sub,
    Unsub,
    Pub,
}
pub(crate) fn message_request(label: &str, action: Option<MessageAction>, data: Option<Vec<u8>>) -> session::Request {
    let req = match action.unwrap_or(MessageAction::Sub) {
        MessageAction::StartPub => session::request::message_channel::Request::StartPub(session::request::message_channel::StartPublish {}),
        MessageAction::StopPub => session::request::message_channel::Request::StopPub(session::request::message_channel::StopPublish {}),
        MessageAction::Sub => session::request::message_channel::Request::Sub(session::request::message_channel::Subscribe {}),
        MessageAction::Unsub => session::request::message_channel::Request::Unsub(session::request::message_channel::Unsubscribe {}),
        MessageAction::Pub => session::request::message_channel::Request::Pub(session::request::message_channel::Publish { data: data.unwrap_or_default() }),
    };
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::MessageChannel(session::request::MessageChannel {
            label: label.into(),
            request: Some(req),
        })),
    }
}
pub(crate) fn feature_request(sources: Vec<Source>, attach: bool) -> session::Request {
    let src = sources.into_iter().map(|s| shared::receiver::Source { peer: s.peer, track: s.track }).collect();
    let action = if attach {
        features::mixer::request::Request::Attach(features::mixer::request::Attach { sources: src })
    } else {
        features::mixer::request::Request::Detach(features::mixer::request::Detach { sources: src })
    };
    let mixer = features::mixer::Request { request: Some(action) };
    let feature = features::Request {
        request: Some(features::request::Request::Mixer(mixer)),
    };
    session::Request {
        req_id: 0,
        request: Some(session::request::Request::Features(feature)),
    }
}
pub(crate) fn make_join_proto(j: &JoinConfig) -> session::RoomJoin {
    join_proto(j)
}
