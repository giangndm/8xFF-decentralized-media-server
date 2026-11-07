//! Manual mixer sources are explicitly attached, detached, and reattached.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command, MixerConfig, Source};
use media_server_protocol::protobuf::features::mixer::Mode;

pub fn config() -> MixerConfig {
    MixerConfig {
        mode: Mode::Manual as i32,
        outputs: 3,
        sources: vec![],
    }
}
pub async fn attach(runtime: &mut BrowserRuntime, source: Source) -> Result<(), ClientError> {
    runtime.command(Command::MixerAttach { sources: vec![source] }).await.map(|_| ())
}
pub async fn cycle(runtime: &mut BrowserRuntime, source: Source) -> Result<(), ClientError> {
    runtime.command(Command::MixerDetach { sources: vec![source.clone()] }).await?;
    attach(runtime, source).await
}
