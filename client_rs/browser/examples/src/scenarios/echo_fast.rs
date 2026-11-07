//! Echo fast: publish two tracks, attach a remote pair, and exchange a room message.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command};

pub async fn setup(runtime: &mut BrowserRuntime) -> Result<(), ClientError> {
    runtime
        .command(Command::CreateMessageChannel {
            label: "testing".into(),
            publish: true,
        })
        .await?;
    runtime
        .command(Command::PublishMessage {
            label: "testing".into(),
            data: b"client-rs echo_fast".to_vec(),
        })
        .await?;
    Ok(())
}
