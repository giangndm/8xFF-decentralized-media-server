mod common;
use atm0s_media_client_core::*;
use common::*;
use media_server_protocol::protobuf::session;

fn completion_count(outputs: &[Output], operation: OperationId) -> usize {
    outputs
        .iter()
        .filter(|output| matches!(output, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == operation))
        .count()
}
fn next_effect(client: &mut Client) -> EffectEnvelope {
    outputs(client)
        .into_iter()
        .find_map(|output| match output {
            Output::Effect(effect) => Some(effect),
            _ => None,
        })
        .expect("next effect")
}
fn connect_command(offer: Option<String>) -> Command {
    Command::Connect {
        gateway: "http://gateway".into(),
        token: "token".into(),
        offer,
        version: "regression".into(),
        join: None,
    }
}

#[test]
fn timed_out_connect_ignores_late_channel_and_can_retry() {
    let (mut client, operation, _) = start();
    client.on_input(60_000, Input::Tick);
    let mut seen = outputs(&mut client);
    assert_eq!(client.snapshot().session, SessionState::Failed);
    client.on_input(
        60_001,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::DataChannelOpen,
        }),
    );
    seen.extend(outputs(&mut client));
    assert_eq!(completion_count(&seen, operation), 1);
    assert_eq!(client.snapshot().session, SessionState::Failed);
    assert!(client.command(60_002, connect_command(Some("retry".into()))).is_ok());
}

#[test]
fn connection_waiting_for_channel_has_deadline_and_disconnect_settles_it() {
    let (mut client, operation, initial) = start();
    for effect in initial {
        feed(
            &mut client,
            &effect,
            EffectResult::Conn {
                conn_id: "id".into(),
                sdp: "answer".into(),
                ice_lite: true,
            },
        );
    }
    let local = next_effect(&mut client);
    feed(&mut client, &local, EffectResult::None);
    let remote = next_effect(&mut client);
    feed(&mut client, &remote, EffectResult::None);
    assert!(client.next_deadline().is_some());
    outputs(&mut client);
    client.command(3, Command::Disconnect).unwrap();
    assert_eq!(completion_count(&outputs(&mut client), operation), 1);
    assert_eq!(client.next_deadline(), None);
}

fn sender_client() -> Client {
    let mut client = connected();
    let operation = client
        .command(
            3,
            Command::CreateSender {
                name: "mic".into(),
                kind: Kind::Audio,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
    let add = next_effect(&mut client);
    client.on_input(
        4,
        Input::EffectCompleted(EffectCompletion {
            id: add.id,
            generation: add.generation,
            result: Err(ClientError::Transport("add failed".into())),
        }),
    );
    assert_eq!(completion_count(&outputs(&mut client), operation), 1);
    client
}

#[test]
fn deferred_attach_timeout_and_disconnect_settle_original_operation() {
    for disconnect in [false, true] {
        let mut client = sender_client();
        let operation = client
            .command(
                5,
                Command::AttachSender {
                    name: "mic".into(),
                    source_id: "track".into(),
                },
            )
            .unwrap();
        let replace = next_effect(&mut client);
        assert_eq!(replace.operation, Some(operation));
        if disconnect {
            client.command(6, Command::Disconnect).unwrap();
        } else {
            client.on_input(60_000, Input::Tick);
        }
        let mut seen = outputs(&mut client);
        feed(&mut client, &replace, EffectResult::None);
        seen.extend(outputs(&mut client));
        assert_eq!(completion_count(&seen, operation), 1);
        assert!(!seen.iter().any(|output| matches!(output, Output::Effect(effect) if matches!(effect.effect, Effect::SendData { .. }))));
    }
}

#[test]
fn sender_attach_preserves_screen_and_metadata() {
    let mut client = sender_client();
    client
        .command(
            5,
            Command::AttachSenderWithMetadata {
                name: "mic".into(),
                source_id: "screen".into(),
                screen: true,
                metadata: Some("presentation".into()),
            },
        )
        .unwrap();
    let replace = next_effect(&mut client);
    feed(&mut client, &replace, EffectResult::None);
    let (_, request) = take_request(&mut client);
    let Some(session::request::Request::Sender(sender)) = request.request else {
        panic!("sender request")
    };
    let Some(session::request::sender::Request::Attach(attach)) = sender.request else {
        panic!("attach request")
    };
    let source = attach.source.unwrap();
    assert!(source.screen);
    assert_eq!(source.metadata.as_deref(), Some("presentation"));
}

#[test]
fn queue_pressure_rejects_commands_without_dropping_existing_results() {
    let mut client = Client::new(ClientConfig {
        max_outputs: 1,
        ..ClientConfig::default()
    });
    let operation = client
        .command(
            0,
            Command::CreateSender {
                name: "one".into(),
                kind: Kind::Audio,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
    assert_eq!(
        client.command(
            0,
            Command::CreateReceiver {
                name: "rejected".into(),
                kind: Kind::Audio,
                config: ReceiverConfig::default()
            }
        ),
        Err(ClientError::QueueFull)
    );
    assert!(client.snapshot().receivers.is_empty());
    assert_eq!(completion_count(&outputs(&mut client), operation), 1);
    let operation = client.command(0, connect_command(Some("offer".into()))).unwrap();
    let seen = outputs(&mut client);
    assert_eq!(completion_count(&seen, operation), 1);
    assert!(seen.iter().any(|output| matches!(
        output,
        Output::Event(ClientEvent::OperationCompleted {
            result: Err(ClientError::QueueFull),
            ..
        })
    )));
}

#[test]
fn automatic_connect_prepares_media_before_offer_and_waits_for_both_ready_signals() {
    let mut client = Client::new(ClientConfig::default());
    client
        .command(
            0,
            Command::CreateSender {
                name: "mic".into(),
                kind: Kind::Audio,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
    outputs(&mut client);
    client
        .command(
            0,
            Command::AttachSender {
                name: "mic".into(),
                source_id: "track".into(),
            },
        )
        .unwrap();
    outputs(&mut client);
    let operation = client.command(1, connect_command(None)).unwrap();
    for expected in ["peer", "transceiver", "track", "offer"] {
        let effect = next_effect(&mut client);
        let actual = match effect.effect {
            Effect::CreatePeer { .. } => "peer",
            Effect::AddTransceiver { .. } => "transceiver",
            Effect::ReplaceTrack { .. } => "track",
            Effect::CreateOffer { .. } => "offer",
            _ => panic!("unexpected preparation"),
        };
        assert_eq!(actual, expected);
        assert_eq!(effect.operation, Some(operation));
        feed(
            &mut client,
            &effect,
            if expected == "offer" {
                EffectResult::Sdp("generated".into())
            } else {
                EffectResult::None
            },
        );
    }
    let http = next_effect(&mut client);
    assert!(matches!(http.effect, Effect::Http(_)));
    feed(
        &mut client,
        &http,
        EffectResult::Conn {
            conn_id: "id".into(),
            sdp: "answer".into(),
            ice_lite: true,
        },
    );
    let local = next_effect(&mut client);
    feed(&mut client, &local, EffectResult::None);
    let remote = next_effect(&mut client);
    client.on_input(
        3,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::DataChannelOpen,
        }),
    );
    assert_eq!(completion_count(&outputs(&mut client), operation), 0);
    feed(&mut client, &remote, EffectResult::None);
    assert_eq!(completion_count(&outputs(&mut client), operation), 1);
    assert_eq!(client.snapshot().session, SessionState::Connected);
}

#[test]
fn mixer_slot_events_preserve_slot_and_source() {
    use media_server_protocol::protobuf::{features, shared};
    use prost::Message;
    let mut client = connected();
    for event in [
        features::mixer::server_event::Event::SlotSet(features::mixer::server_event::SlotSet {
            slot: 2,
            source: Some(shared::receiver::Source {
                peer: "speaker".into(),
                track: "mic".into(),
            }),
        }),
        features::mixer::server_event::Event::SlotUnset(features::mixer::server_event::SlotUnset { slot: 2 }),
    ] {
        let bytes = session::ServerEvent {
            seq: 1,
            event: Some(session::server_event::Event::Features(features::ServerEvent {
                event: Some(features::server_event::Event::Mixer(features::mixer::ServerEvent { event: Some(event) })),
            })),
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
    let events: Vec<_> = outputs(&mut client)
        .into_iter()
        .filter_map(|output| match output {
            Output::Event(ClientEvent::Mixer(event)) => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        events,
        vec![
            MixerEvent::OutputChanged {
                slot: 2,
                source: Some(Source {
                    peer: "speaker".into(),
                    track: "mic".into()
                })
            },
            MixerEvent::OutputChanged { slot: 2, source: None },
        ]
    );
}

#[test]
fn preparation_errors_settle_connect_and_remove_all_continuations() {
    for fail_at in ["peer", "transceiver", "track", "offer"] {
        let mut client = Client::new(ClientConfig::default());
        client
            .command(
                0,
                Command::CreateSender {
                    name: "mic".into(),
                    kind: Kind::Audio,
                    config: SenderConfig::default(),
                },
            )
            .unwrap();
        outputs(&mut client);
        client
            .command(
                0,
                Command::AttachSender {
                    name: "mic".into(),
                    source_id: "track".into(),
                },
            )
            .unwrap();
        outputs(&mut client);
        let operation = client.command(1, connect_command(None)).unwrap();
        for stage in ["peer", "transceiver", "track", "offer"] {
            let effect = next_effect(&mut client);
            if stage == fail_at {
                client.on_input(
                    2,
                    Input::EffectCompleted(EffectCompletion {
                        id: effect.id,
                        generation: effect.generation,
                        result: Err(ClientError::Transport("preparation failed".into())),
                    }),
                );
                let seen = outputs(&mut client);
                assert_eq!(completion_count(&seen, operation), 1);
                assert!(!seen.iter().any(|output| matches!(output, Output::Effect(effect) if !matches!(effect.effect, Effect::ClosePeer { .. }))));
                assert_eq!(client.next_deadline(), None);
                assert_eq!(client.snapshot().session, SessionState::Failed);
                break;
            }
            feed(&mut client, &effect, EffectResult::None);
        }
    }
}

#[test]
fn negotiation_failure_and_timeout_release_the_negotiation_lock() {
    for timeout in [false, true] {
        let mut client = connected();
        let operation = client
            .command(
                4,
                Command::CreateReceiver {
                    name: "one".into(),
                    kind: Kind::Video,
                    config: ReceiverConfig::default(),
                },
            )
            .unwrap();
        let add = next_effect(&mut client);
        if timeout {
            client.on_input(60_000, Input::Tick);
        } else {
            client.on_input(
                5,
                Input::EffectCompleted(EffectCompletion {
                    id: add.id,
                    generation: add.generation,
                    result: Err(ClientError::Transport("add failed".into())),
                }),
            );
        }
        assert_eq!(completion_count(&outputs(&mut client), operation), 1);
        assert!(client
            .command(
                60_001,
                Command::CreateReceiver {
                    name: "two".into(),
                    kind: Kind::Video,
                    config: ReceiverConfig::default()
                }
            )
            .is_ok());
    }
}

#[test]
fn channel_creation_waits_for_subscription_and_publication_acknowledgements() {
    for fail_subscription in [false, true] {
        let mut client = connected();
        let operation = client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: true }).unwrap();
        let (subscribe, request) = take_request(&mut client);
        feed(&mut client, &subscribe, EffectResult::None);
        assert!(outputs(&mut client).is_empty());
        if fail_subscription {
            response(
                &mut client,
                request.req_id,
                session::response::Response::Error(media_server_protocol::protobuf::shared::Error { code: 403, message: "denied".into() }),
            );
            let seen = outputs(&mut client);
            assert_eq!(completion_count(&seen, operation), 1);
            assert!(!seen.iter().any(|output| matches!(output, Output::Effect(_))));
        } else {
            response(
                &mut client,
                request.req_id,
                session::response::Response::MessageChannel(session::response::MessageChannel {
                    label: "chat".into(),
                    response: Some(session::response::message_channel::Response::Sub(session::response::message_channel::Subscribe {})),
                }),
            );
            let (publish, request) = take_request(&mut client);
            assert_eq!(publish.operation, Some(operation));
            feed(&mut client, &publish, EffectResult::None);
            assert!(outputs(&mut client).is_empty());
            response(
                &mut client,
                request.req_id,
                session::response::Response::MessageChannel(session::response::MessageChannel {
                    label: "chat".into(),
                    response: Some(session::response::message_channel::Response::StartPub(session::response::message_channel::StartPublish {})),
                }),
            );
            assert_eq!(completion_count(&outputs(&mut client), operation), 1);
        }
    }
}

#[test]
fn automatic_restart_generates_offer_then_waits_for_applied_answer() {
    use media_server_protocol::protobuf::gateway;
    use prost::Message;
    let mut client = connected();
    let operation = client.command(4, Command::RestartIce { offer: None }).unwrap();
    let offer = next_effect(&mut client);
    assert!(matches!(offer.effect, Effect::CreateOffer { restart: true, .. }));
    feed(&mut client, &offer, EffectResult::Sdp("generated-restart".into()));
    let http = next_effect(&mut client);
    let Effect::Http(request) = &http.effect else { panic!("restart HTTP") };
    assert!(request.url.ends_with("/connection-a/restart-ice"));
    assert_eq!(gateway::ConnectRequest::decode(request.body.as_slice()).unwrap().sdp, "generated-restart");
    feed(
        &mut client,
        &http,
        EffectResult::Conn {
            conn_id: "connection-b".into(),
            sdp: "restart-answer".into(),
            ice_lite: true,
        },
    );
    let local = next_effect(&mut client);
    assert!(matches!(&local.effect, Effect::SetLocalDescription { sdp, .. } if sdp == "generated-restart"));
    feed(&mut client, &local, EffectResult::None);
    let remote = next_effect(&mut client);
    assert!(matches!(&remote.effect, Effect::SetRemoteDescription { sdp, .. } if sdp == "restart-answer"));
    assert_eq!(client.snapshot().session, SessionState::Reconnecting);
    feed(&mut client, &remote, EffectResult::None);
    assert_eq!(completion_count(&outputs(&mut client), operation), 1);
    assert_eq!(client.snapshot().session, SessionState::Connected);
}

#[test]
fn automatic_restart_offer_failure_settles_and_closes_peer() {
    let mut client = connected();
    let operation = client.command(4, Command::RestartIce { offer: None }).unwrap();
    let offer = next_effect(&mut client);
    client.on_input(
        5,
        Input::EffectCompleted(EffectCompletion {
            id: offer.id,
            generation: offer.generation,
            result: Err(ClientError::Transport("offer failed".into())),
        }),
    );
    let seen = outputs(&mut client);
    assert_eq!(completion_count(&seen, operation), 1);
    assert!(seen.iter().any(|output| matches!(output, Output::Effect(EffectEnvelope { effect: Effect::ClosePeer { .. }, .. }))));
    assert_eq!(client.next_deadline(), None);
    assert_eq!(client.snapshot().session, SessionState::Failed);
}

fn candidate(client: &mut Client, value: &str) {
    client.on_input(
        2,
        Input::Rtc(RtcEventEnvelope {
            generation: client.generation(),
            event: RtcEvent::LocalCandidate(value.into()),
        }),
    );
}
fn candidate_requests(outputs: &[Output]) -> Vec<&HttpRequest> {
    outputs
        .iter()
        .filter_map(|output| match output {
            Output::Effect(EffectEnvelope { effect: Effect::Http(request), .. }) if request.url.ends_with("/ice-candidate") => Some(request),
            _ => None,
        })
        .collect()
}

#[test]
fn initial_candidates_wait_for_answer_then_use_new_connection_and_ice_mode() {
    use media_server_protocol::protobuf::gateway;
    use prost::Message;
    for ice_lite in [false, true] {
        let (mut client, _, initial) = start();
        candidate(&mut client, "early-candidate");
        assert!(candidate_requests(&outputs(&mut client)).is_empty());
        for effect in initial {
            feed(
                &mut client,
                &effect,
                EffectResult::Conn {
                    conn_id: "new-id".into(),
                    sdp: "answer".into(),
                    ice_lite,
                },
            );
        }
        let seen = outputs(&mut client);
        let requests = candidate_requests(&seen);
        if ice_lite {
            assert!(requests.is_empty());
        } else {
            assert_eq!(requests.len(), 1);
            assert!(requests[0].url.ends_with("/new-id/ice-candidate"));
            assert_eq!(gateway::RemoteIceRequest::decode(requests[0].body.as_slice()).unwrap().candidates, vec!["early-candidate"]);
        }
    }
}

#[test]
fn restart_candidates_never_use_previous_connection_or_ice_mode() {
    use media_server_protocol::protobuf::gateway;
    use prost::Message;
    for new_ice_lite in [false, true] {
        let (mut client, _, initial) = start();
        for effect in initial {
            feed(
                &mut client,
                &effect,
                EffectResult::Conn {
                    conn_id: "previous-id".into(),
                    sdp: "answer".into(),
                    ice_lite: !new_ice_lite,
                },
            );
        }
        let local = next_effect(&mut client);
        feed(&mut client, &local, EffectResult::None);
        let remote = next_effect(&mut client);
        feed(&mut client, &remote, EffectResult::None);
        client.on_input(
            2,
            Input::Rtc(RtcEventEnvelope {
                generation: client.generation(),
                event: RtcEvent::DataChannelOpen,
            }),
        );
        outputs(&mut client);
        client.command(4, Command::RestartIce { offer: Some("restart".into()) }).unwrap();
        let http = next_effect(&mut client);
        candidate(&mut client, "during-restart");
        assert!(outputs(&mut client).is_empty(), "candidate must wait for the restart response");
        feed(
            &mut client,
            &http,
            EffectResult::Conn {
                conn_id: "restarted-id".into(),
                sdp: "answer".into(),
                ice_lite: new_ice_lite,
            },
        );
        let seen = outputs(&mut client);
        let requests = candidate_requests(&seen);
        if new_ice_lite {
            assert!(requests.is_empty());
        } else {
            assert_eq!(requests.len(), 1);
            assert!(requests[0].url.ends_with("/restarted-id/ice-candidate"));
            assert_eq!(gateway::RemoteIceRequest::decode(requests[0].body.as_slice()).unwrap().candidates, vec!["during-restart"]);
        }
        candidate(&mut client, "after-answer");
        let seen = outputs(&mut client);
        let requests = candidate_requests(&seen);
        assert_eq!(requests.len(), usize::from(!new_ice_lite));
        assert!(requests.iter().all(|request| request.url.ends_with("/restarted-id/ice-candidate")));
    }
}
