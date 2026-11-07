//! Golden wire payloads were encoded by protoc, independently of prost.
use atm0s_media_server_protocol::protobuf::{gateway, session, shared};
use prost::Message;

#[test]
fn decodes_and_reencodes_connect_with_optional_zero_and_metadata() {
    let bytes = include_bytes!("client_fixtures/connect.bin");
    let request = gateway::ConnectRequest::decode(bytes.as_slice()).unwrap();
    assert_eq!(request.version, "rust-fixture");
    let join = request.join.as_ref().unwrap();
    assert_eq!(join.metadata.as_deref(), Some("metadata"));
    let tracks = request.tracks.as_ref().unwrap();
    assert_eq!(tracks.senders[0].kind, shared::Kind::Video as i32);
    assert!(tracks.senders[0].state.as_ref().unwrap().source.as_ref().unwrap().screen);
    let receiver = tracks.receivers[0].state.as_ref().unwrap().config.as_ref().unwrap();
    assert_eq!(receiver.min_spatial, Some(0));
    assert_eq!(receiver.min_temporal, Some(0));
    assert_eq!(request.encode_to_vec(), bytes);
}

#[test]
fn preserves_binary_room_message_and_request_identity() {
    let bytes = include_bytes!("client_fixtures/client_message.bin");
    let event = session::ClientEvent::decode(bytes.as_slice()).unwrap();
    assert_eq!(event.seq, 42);
    let Some(session::client_event::Event::Request(request)) = &event.event else {
        panic!("expected request")
    };
    assert_eq!(request.req_id, 7);
    let Some(session::request::Request::MessageChannel(channel)) = &request.request else {
        panic!("expected message channel")
    };
    assert_eq!(channel.label, "bytes");
    let Some(session::request::message_channel::Request::Pub(message)) = &channel.request else {
        panic!("expected publication")
    };
    assert_eq!(message.data, [0, 1, 127, 128, 255]);
    assert_eq!(event.encode_to_vec(), bytes);
}

#[test]
fn preserves_server_error_code_and_correlation() {
    let bytes = include_bytes!("client_fixtures/server_error.bin");
    let event = session::ServerEvent::decode(bytes.as_slice()).unwrap();
    let Some(session::server_event::Event::Response(response)) = &event.event else {
        panic!("expected response")
    };
    assert_eq!(response.req_id, 7);
    let Some(session::response::Response::Error(error)) = &response.response else {
        panic!("expected error")
    };
    assert_eq!(error.code, 403);
    assert_eq!(error.message, "permission denied");
    assert_eq!(event.encode_to_vec(), bytes);
}
