#[cfg(test)]
mod tests {
    use atm0s_media_client_core::*;
    #[test]
    fn creates_tracks_and_connect_effect() {
        let mut c = Client::new(ClientConfig::default());
        c.command(
            0,
            Command::CreateSender {
                name: "audio".into(),
                kind: Kind::Audio,
                config: SenderConfig::default(),
            },
        )
        .unwrap();
        c.command(
            0,
            Command::Connect {
                gateway: "http://gw".into(),
                token: "secret".into(),
                offer: Some("v=0".into()),
                version: "test".into(),
                join: None,
            },
        )
        .unwrap();
        assert_eq!(c.snapshot().session, SessionState::Connecting);
        assert!(matches!(c.poll_output(), Some(Output::Event(_))));
    }
    #[test]
    fn timeout_settles_effect() {
        let mut c = Client::new(ClientConfig {
            rpc_timeout_ms: 5,
            ..Default::default()
        });
        let op = c
            .command(
                0,
                Command::Connect {
                    gateway: "http://gw".into(),
                    token: "x".into(),
                    offer: Some("v=0".into()),
                    version: "test".into(),
                    join: None,
                },
            )
            .unwrap();
        for _ in 0..3 {
            c.poll_output();
        }
        c.on_input(10, Input::Tick);
        let mut found = false;
        while let Some(output) = c.poll_output() {
            if let Output::Event(ClientEvent::OperationCompleted { id, result }) = output {
                if id == op {
                    found = result.is_err();
                }
            }
        }
        assert!(found);
    }
}
