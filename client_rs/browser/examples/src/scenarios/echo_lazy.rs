//! Connect before declaring tracks and joining; later leave and rejoin the same room.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command, JoinConfig};

pub async fn cycle(runtime: &mut BrowserRuntime, info: JoinConfig, token: String) -> Result<(), ClientError> {
    runtime.command(Command::Leave).await?;
    runtime.command(Command::Join { info, token }).await.map(|_| ())
}
