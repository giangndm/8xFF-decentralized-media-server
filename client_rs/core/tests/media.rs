mod common;
use atm0s_media_client_core::*;
use common::*;
use media_server_protocol::protobuf::session;
use prost::Message;

#[test]
fn configure_before_connect_is_in_wire_snapshot() {
    let mut client = Client::new(ClientConfig::default());
    client
        .command(
            0,
            Command::CreateSender {
                name: "video".into(),
                kind: Kind::Video,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
    client
        .command(
            0,
            Command::ConfigureSender {
                name: "video".into(),
                config: SenderConfig { priority: 99, ..Default::default() },
            },
        )
        .unwrap();
    assert_eq!(client.snapshot().senders[0].config.priority, 99);
    outputs(&mut client);
    client
        .command(
            1,
            Command::Connect {
                gateway: "http://gateway".into(),
                token: "test-token".into(),
                offer: Some(INITIAL_OFFER.into()),
                version: "test".into(),
                join: None,
            },
        )
        .unwrap();
    let request = outputs(&mut client)
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(EffectEnvelope { effect: Effect::Http(request), .. }) => Some(request),
            _ => None,
        })
        .expect("connect HTTP request");
    let connect = media_server_protocol::protobuf::gateway::ConnectRequest::decode(request.body.as_slice()).unwrap();
    let sender = connect.tracks.unwrap().senders.into_iter().find(|sender| sender.name == "video").unwrap();
    assert_eq!(sender.state.unwrap().config.unwrap().priority, 99);
}

#[test]
fn local_candidate_is_sent_to_the_current_non_lite_connection() {
    let mut client = connected();
    // A restart response can change the server's ICE mode and connection ID.
    client.command(4, Command::RestartIce { offer: Some("restart-offer".into()) }).unwrap();
    let effects: Vec<_> = outputs(&mut client)
        .into_iter()
        .filter_map(|o| {
            if let Output::Effect(e) = o {
                Some(e)
            } else {
                None
            }
        })
        .collect();
    for effect in effects {
        feed(
            &mut client,
            &effect,
            EffectResult::Conn {
                conn_id: "connection-b".into(),
                sdp: "restart-answer".into(),
                ice_lite: false,
            },
        );
    }
    outputs(&mut client);
    client.on_input(
        5,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::LocalCandidate("candidate:example".into()),
        }),
    );
    let request = outputs(&mut client)
        .into_iter()
        .find_map(|o| match o {
            Output::Effect(EffectEnvelope { effect: Effect::Http(request), .. }) => Some(request),
            _ => None,
        })
        .expect("non-lite candidate HTTP request");
    assert_eq!(request.url, "http://gateway/webrtc/connection-b/ice-candidate");
    let payload = media_server_protocol::protobuf::gateway::RemoteIceRequest::decode(request.body.as_slice()).unwrap();
    assert_eq!(payload.candidates, ["candidate:example"]);
}

#[test]
fn attaching_sender_replaces_browser_track_then_waits_for_server_ack() {
    let mut client = connected();
    client
        .command(
            3,
            Command::CreateSender {
                name: "camera".into(),
                kind: Kind::Video,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
    outputs(&mut client);
    let op = client
        .command(
            4,
            Command::AttachSender {
                name: "camera".into(),
                source_id: "camera-track-1".into(),
            },
        )
        .unwrap();
    let effect = outputs(&mut client)
        .into_iter()
        .find_map(|o| match o {
            Output::Effect(e) if matches!(e.effect, Effect::ReplaceTrack { .. }) => Some(e),
            _ => None,
        })
        .expect("replaceTrack before server attach");
    feed(&mut client, &effect, EffectResult::None);
    let (send, request) = take_request(&mut client);
    let Some(session::request::Request::Sender(sender)) = request.request else {
        panic!("sender control request")
    };
    let Some(session::request::sender::Request::Attach(attach)) = sender.request else {
        panic!("sender attach")
    };
    assert_eq!(sender.name, "camera");
    assert_eq!(attach.source.unwrap().id, "camera-track-1");
    feed(&mut client, &send, EffectResult::None);
    assert!(!outputs(&mut client).iter().any(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == op)));
    response(
        &mut client,
        request.req_id,
        session::response::Response::Sender(session::response::Sender {
            response: Some(session::response::sender::Response::Attach(session::response::sender::Attach {})),
        }),
    );
    assert_eq!(
        outputs(&mut client)
            .iter()
            .filter(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, result: Ok(_) }) if *id == op))
            .count(),
        1
    );
}
