#![allow(dead_code)]

use atm0s_media_client_core::*;
use media_server_protocol::protobuf::session;
use prost::Message;

pub const INITIAL_OFFER: &str = "v=0\r\ns=unique-initial-offer\r\n";

pub fn outputs(client: &mut Client) -> Vec<Output> {
    std::iter::from_fn(|| client.poll_output()).collect()
}

pub fn feed(client: &mut Client, effect: &EffectEnvelope, result: EffectResult) {
    client.on_input(
        1,
        Input::EffectCompleted(EffectCompletion {
            id: effect.id,
            generation: effect.generation,
            result: Ok(result),
        }),
    );
}

pub fn start() -> (Client, OperationId, Vec<EffectEnvelope>) {
    let mut client = Client::new(ClientConfig::default());
    let op = client
        .command(
            0,
            Command::Connect {
                gateway: "http://gateway".into(),
                token: "test-token".into(),
                offer: Some(INITIAL_OFFER.into()),
                version: "test".into(),
                join: None,
            },
        )
        .unwrap();
    let effects = outputs(&mut client)
        .into_iter()
        .filter_map(|o| match o {
            Output::Effect(e) => Some(e),
            _ => None,
        })
        .collect();
    (client, op, effects)
}

pub fn connected() -> Client {
    let (mut client, _, initial) = start();
    for effect in initial {
        let result = match &effect.effect {
            Effect::Http(_) => EffectResult::Conn {
                conn_id: "connection-a".into(),
                sdp: "answer".into(),
                ice_lite: true,
            },
            Effect::CreateOffer { .. } => EffectResult::Sdp("offer".into()),
            _ => EffectResult::None,
        };
        feed(&mut client, &effect, result);
    }
    for _ in 0..10 {
        let next = outputs(&mut client);
        if next.is_empty() {
            break;
        }
        for output in next {
            if let Output::Effect(effect) = output {
                feed(&mut client, &effect, EffectResult::None);
            }
        }
    }
    client.on_input(
        2,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::DataChannelOpen,
        }),
    );
    outputs(&mut client);
    assert_eq!(client.snapshot().session, SessionState::Connected);
    client
}

pub fn response(client: &mut Client, req_id: u32, payload: session::response::Response) {
    let bytes = session::ServerEvent {
        seq: 1,
        event: Some(session::server_event::Event::Response(session::Response { req_id, response: Some(payload) })),
    }
    .encode_to_vec();
    client.on_input(
        3,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::Data(bytes),
        }),
    );
}

pub fn take_request(client: &mut Client) -> (EffectEnvelope, session::Request) {
    let effect = outputs(client)
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(e) if matches!(e.effect, Effect::SendData { .. }) => Some(e),
            _ => None,
        })
        .expect("RPC should produce SendData");
    let Effect::SendData { data, .. } = &effect.effect else { unreachable!() };
    let event = session::ClientEvent::decode(data.as_slice()).unwrap();
    let Some(session::client_event::Event::Request(request)) = event.event else { panic!("request") };
    (effect, request)
}
