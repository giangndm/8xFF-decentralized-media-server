mod common;
use atm0s_media_client_core::*;
use common::*;
use media_server_protocol::protobuf::session;

#[test]
fn dynamic_receiver_waits_for_offer_server_answer_and_remote_description() {
    let mut client = connected();
    let operation = client
        .command(
            4,
            Command::CreateReceiver {
                name: "extra".into(),
                kind: Kind::Video,
                config: ReceiverConfig::default(),
            },
        )
        .unwrap();
    let mut effects: Vec<_> = outputs(&mut client)
        .into_iter()
        .filter_map(|output| match output {
            Output::Effect(effect) => Some(effect),
            _ => None,
        })
        .collect();
    let add = effects.iter().find(|effect| matches!(effect.effect, Effect::AddTransceiver { .. })).expect("add receiver transceiver");
    feed(&mut client, add, EffectResult::None);
    effects.extend(outputs(&mut client).into_iter().filter_map(|output| match output {
        Output::Effect(effect) => Some(effect),
        _ => None,
    }));
    let offer = effects.iter().find(|effect| matches!(effect.effect, Effect::CreateOffer { .. })).expect("create new offer");
    feed(&mut client, offer, EffectResult::Sdp("dynamic-offer".into()));
    let pending = outputs(&mut client);
    assert!(!pending
        .iter()
        .any(|output| matches!(output, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == operation)));
    let local = pending
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(effect) if matches!(&effect.effect, Effect::SetLocalDescription { sdp, .. } if sdp == "dynamic-offer") => Some(effect),
            _ => None,
        })
        .expect("apply generated local offer");
    feed(&mut client, &local, EffectResult::None);
    let (send, request) = take_request(&mut client);
    assert!(matches!(
        &request.request,
        Some(session::request::Request::Session(session::request::Session {
            request: Some(session::request::session::Request::Sdp(_))
        }))
    ));
    feed(&mut client, &send, EffectResult::None);
    response(
        &mut client,
        request.req_id,
        session::response::Response::Session(session::response::Session {
            response: Some(session::response::session::Response::Sdp(session::response::session::UpdateSdp { sdp: "dynamic-answer".into() })),
        }),
    );
    let pending = outputs(&mut client);
    assert!(!pending
        .iter()
        .any(|output| matches!(output, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == operation)));
    let remote = pending
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(effect) if matches!(&effect.effect, Effect::SetRemoteDescription { sdp, .. } if sdp == "dynamic-answer") => Some(effect),
            _ => None,
        })
        .expect("apply negotiated answer");
    feed(&mut client, &remote, EffectResult::None);
    let completions: Vec<_> = outputs(&mut client)
        .into_iter()
        .filter_map(|output| match output {
            Output::Event(ClientEvent::OperationCompleted { id, result }) if id == operation => Some(result),
            _ => None,
        })
        .collect();
    assert!(matches!(completions.as_slice(), [Ok(OperationResult::Updated)]));
}
