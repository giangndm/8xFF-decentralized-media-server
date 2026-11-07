mod common;
use atm0s_media_client_core::*;
use common::*;

#[test]
fn restart_does_not_report_success_before_applying_answer() {
    let mut client = connected();
    let op = client.command(4, Command::RestartIce { offer: Some("restart-offer".into()) }).unwrap();
    let http = outputs(&mut client)
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(effect) if matches!(effect.effect, Effect::Http(_)) => Some(effect),
            _ => None,
        })
        .unwrap();
    feed(
        &mut client,
        &http,
        EffectResult::Conn {
            conn_id: "restarted".into(),
            sdp: "restart-answer".into(),
            ice_lite: true,
        },
    );
    let pending = outputs(&mut client);
    assert!(
        !pending
            .iter()
            .any(|output| matches!(output, Output::Event(ClientEvent::OperationCompleted { id, result: Ok(_) }) if *id == op)),
        "HTTP answer alone cannot prove the browser applied its SDP"
    );
}

#[test]
fn connect_does_not_succeed_on_http_answer_alone() {
    let (mut client, op, initial) = start();
    for effect in initial {
        feed(
            &mut client,
            &effect,
            EffectResult::Conn {
                conn_id: "connection-a".into(),
                sdp: "answer".into(),
                ice_lite: true,
            },
        );
    }
    assert_eq!(client.snapshot().session, SessionState::Connecting);
    assert!(!outputs(&mut client)
        .iter()
        .any(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, result: Ok(_) }) if *id == op)));
}

#[test]
fn disconnect_settles_pending_and_always_closes_local_peer() {
    let mut client = connected();
    let pending = client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: false }).unwrap();
    let (effect, _) = take_request(&mut client);
    feed(&mut client, &effect, EffectResult::None);
    outputs(&mut client);
    client.command(4, Command::Disconnect).unwrap();
    let mut seen = outputs(&mut client);
    client.on_input(60_000, Input::Tick);
    seen.extend(outputs(&mut client));
    assert!(seen.iter().any(|o| matches!(o, Output::Effect(EffectEnvelope { effect: Effect::ClosePeer { .. }, .. }))));
    assert_eq!(
        seen.iter()
            .filter(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, result: Err(_) }) if *id == pending))
            .count(),
        1
    );
    assert_eq!(client.snapshot().session, SessionState::Closed);
}

#[test]
fn stale_generation_events_do_not_change_session() {
    let mut client = connected();
    client.on_input(
        10,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation().saturating_sub(1),
            event: RtcEvent::ConnectionState("disconnected".into()),
        }),
    );
    assert_eq!(client.snapshot().session, SessionState::Connected);
    assert!(outputs(&mut client).is_empty());
}

#[test]
fn initial_connect_applies_local_and_remote_descriptions() {
    let (mut client, _, initial) = start();
    let mut descriptions = Vec::new();
    for effect in initial {
        feed(
            &mut client,
            &effect,
            EffectResult::Conn {
                conn_id: "connection-a".into(),
                sdp: "answer".into(),
                ice_lite: true,
            },
        );
    }
    for _ in 0..10 {
        let next = outputs(&mut client);
        if next.is_empty() {
            break;
        }
        for output in next {
            if let Output::Effect(effect) = output {
                match &effect.effect {
                    Effect::SetLocalDescription { sdp, .. } => descriptions.push(("local", sdp.clone())),
                    Effect::SetRemoteDescription { sdp, .. } => descriptions.push(("remote", sdp.clone())),
                    _ => {}
                }
                feed(&mut client, &effect, EffectResult::None);
            }
        }
    }
    assert_eq!(descriptions, [("local", INITIAL_OFFER.into()), ("remote", "answer".into())]);
}

#[test]
fn failed_http_connect_transitions_to_failed_and_closes_peer() {
    let (mut client, _, initial) = start();
    for effect in initial {
        client.on_input(
            1,
            Input::EffectCompleted(EffectCompletion {
                id: effect.id,
                generation: effect.generation,
                result: Err(ClientError::Http {
                    status: 403,
                    body: b"denied".to_vec(),
                }),
            }),
        );
    }
    assert_eq!(client.snapshot().session, SessionState::Failed);
    assert!(outputs(&mut client)
        .iter()
        .any(|o| matches!(o, Output::Effect(EffectEnvelope { effect: Effect::ClosePeer { .. }, .. }))));
}
