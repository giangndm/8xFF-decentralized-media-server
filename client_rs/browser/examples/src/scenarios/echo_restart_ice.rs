//! Restart ICE through the generic client and wait for its applied answer.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command};

pub async fn restart(runtime: &mut BrowserRuntime) -> Result<(), ClientError> {
    runtime.command(Command::RestartIce { offer: None }).await.map(|_| ())
}
