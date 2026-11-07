mod common;
use atm0s_media_client_core::*;
use common::*;
use media_server_protocol::protobuf::session;

#[test]
fn rpc_send_completion_is_not_server_acknowledgement() {
    let mut client = connected();
    let op = client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: false }).unwrap();
    let (effect, _) = take_request(&mut client);
    feed(&mut client, &effect, EffectResult::None);
    assert!(!outputs(&mut client).iter().any(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == op)));
}

#[test]
fn server_rpc_error_preserves_failure_and_completes_once() {
    let mut client = connected();
    let op = client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: false }).unwrap();
    let (effect, request) = take_request(&mut client);
    feed(&mut client, &effect, EffectResult::None);
    outputs(&mut client);
    response(
        &mut client,
        request.req_id,
        session::response::Response::Error(media_server_protocol::protobuf::shared::Error { code: 403, message: "denied".into() }),
    );
    let result = outputs(&mut client);
    assert_eq!(
        result
            .iter()
            .filter(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, result: Err(_) }) if *id == op))
            .count(),
        1
    );
    client.on_input(60_000, Input::Tick);
    assert!(!outputs(&mut client).iter().any(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, .. }) if *id == op)));
}

#[test]
fn wrong_rpc_response_variant_cannot_report_success() {
    let mut client = connected();
    let op = client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: false }).unwrap();
    let (effect, request) = take_request(&mut client);
    feed(&mut client, &effect, EffectResult::None);
    outputs(&mut client);
    response(
        &mut client,
        request.req_id,
        session::response::Response::Receiver(session::response::Receiver {
            response: Some(session::response::receiver::Response::Detach(session::response::receiver::Detach {})),
        }),
    );
    assert!(!outputs(&mut client)
        .iter()
        .any(|o| matches!(o, Output::Event(ClientEvent::OperationCompleted { id, result: Ok(_) }) if *id == op)));
}

#[test]
fn publishing_channel_subscribes_before_starting_publication() {
    let mut client = connected();
    client.command(3, Command::CreateMessageChannel { label: "chat".into(), publish: true }).unwrap();
    let (_, request) = take_request(&mut client);
    let Some(session::request::Request::MessageChannel(channel)) = request.request else {
        panic!("message channel")
    };
    assert!(matches!(channel.request, Some(session::request::message_channel::Request::Sub(_))));
}
