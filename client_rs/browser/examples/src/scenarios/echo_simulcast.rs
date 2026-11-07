//! Simulcast: create the video sender with three active encodings.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::{ClientError, Command, ReceiverConfig, SenderConfig};
use js_sys::{Array, Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

/// Change both publication priority and receiver layer selection after connect.
pub async fn change_layers(runtime: &mut BrowserRuntime) -> Result<(), ClientError> {
    let transceiver = runtime.transceiver("video_main").ok_or_else(|| ClientError::InvalidInput("video sender is missing".into()))?;
    let sender: JsValue = transceiver.sender().into();
    let parameters = Reflect::get(&sender, &"getParameters".into())
        .and_then(|function| function.dyn_into::<Function>())
        .and_then(|function| function.call0(&sender))
        .and_then(|parameters| Reflect::get(&parameters, &"encodings".into()))
        .map_err(|error| ClientError::Transport(format!("simulcast parameters: {error:?}")))?;
    if Array::from(&parameters).length() != 3 {
        return Err(ClientError::Protocol("simulcast requires three negotiated send encodings".into()));
    }
    runtime
        .command(Command::ConfigureSender {
            name: "video_main".into(),
            config: SenderConfig {
                priority: 2,
                bitrate: 800_000,
                simulcast: true,
            },
        })
        .await?;
    runtime
        .command(Command::ConfigureReceiver {
            name: "video_recv".into(),
            config: ReceiverConfig {
                max_spatial: 0,
                max_temporal: 0,
                ..Default::default()
            },
        })
        .await?;
    Ok(())
}
