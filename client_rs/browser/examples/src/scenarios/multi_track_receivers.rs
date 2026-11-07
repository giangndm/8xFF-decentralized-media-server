//! Multiple receivers: add and attach additional remote media receivers.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command, Kind, ReceiverConfig, Source};

pub async fn attach(runtime: &mut BrowserRuntime, name: String, kind: Kind, source: Source) -> Result<(), ClientError> {
    runtime
        .command(Command::CreateReceiver {
            name: name.clone(),
            kind,
            config: ReceiverConfig::default(),
        })
        .await?;
    runtime.command(Command::AttachReceiver { name, source }).await.map(|_| ())
}
