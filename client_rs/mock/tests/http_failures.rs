use atm0s_media_client_core::{Atm0sClient, Client, ClientConfig, ClientError, ClientEvent, Command, Effect, HttpResponse, SessionState};
use atm0s_media_client_mock::{MockHttp, MockPeer};

#[test]
fn http_failures_settle_connect_and_close_peer() {
    let failures = [
        Err(ClientError::Transport("offline".into())),
        Ok(HttpResponse {
            status: 403,
            body: b"denied".to_vec(),
        }),
        Ok(HttpResponse { status: 200, body: vec![0xff] }),
    ];
    for (index, response) in failures.into_iter().enumerate() {
        let mut http = MockHttp::default();
        http.responses.push_back(response);
        let mut client = Atm0sClient::new(Client::new(ClientConfig::default()), http, MockPeer::default());
        let operation = client
            .command(
                0,
                Command::Connect {
                    gateway: "http://gateway".into(),
                    token: "scoped-token".into(),
                    offer: Some("offer".into()),
                    version: "test".into(),
                    join: None,
                },
            )
            .unwrap();
        futures::executor::block_on(client.pump()).unwrap();
        let results: Vec<_> = std::iter::from_fn(|| client.poll_event())
            .filter_map(|event| match event {
                ClientEvent::OperationCompleted { id, result } if id == operation => Some(result),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 1, "failure {index} must complete exactly once");
        match (index, &results[0]) {
            (0, Err(ClientError::Transport(message))) => assert_eq!(message, "offline"),
            (1, Err(ClientError::Http { status, body })) => {
                assert_eq!(*status, 403);
                assert_eq!(body, b"denied");
            }
            (2, Err(ClientError::Protocol(_))) => {}
            _ => panic!("unexpected failure: {:?}", results[0]),
        }
        assert_eq!(client.core.snapshot().session, SessionState::Failed);
        assert!(client.peer.effects.iter().any(|effect| matches!(effect, Effect::ClosePeer { .. })));
    }
}
