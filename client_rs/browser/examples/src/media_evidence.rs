//! Measurements used by example assertions; receiving `ontrack` is insufficient.
use atm0s_media_client_browser::BrowserRuntime;
use atm0s_media_client_core::ClientError;
use js_sys::{Array, Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// Read decoded audio samples from one particular receiver, including a mixer
/// output. Compare two observations to prove audio continues to arrive.
pub async fn decoded_audio_samples(runtime: &BrowserRuntime, name: &str) -> Result<f64, ClientError> {
    let transceiver = runtime.transceiver(name).ok_or_else(|| ClientError::InvalidInput(format!("unknown receiver {name}")))?;
    let receiver = Reflect::get(transceiver.as_ref(), &"receiver".into()).map_err(stats_error)?;
    let get_stats = Reflect::get(&receiver, &"getStats".into()).and_then(|method| method.dyn_into::<Function>()).map_err(stats_error)?;
    let promise = get_stats.call0(&receiver).and_then(|value| value.dyn_into::<Promise>()).map_err(stats_error)?;
    let report = JsFuture::from(promise).await.map_err(stats_error)?;
    let entries = js_sys::try_iter(&report)
        .map_err(stats_error)?
        .ok_or_else(|| ClientError::Protocol("receiver stats report is not iterable".into()))?;
    let mut samples = 0.0;
    for entry in entries {
        let record = Array::from(&entry.map_err(stats_error)?).get(1);
        if string(&record, "type").as_deref() != Some("inbound-rtp") {
            continue;
        }
        let kind = string(&record, "kind").or_else(|| string(&record, "mediaType"));
        if kind.as_deref() == Some("audio") && number(&record, "packetsReceived") > 0.0 {
            samples += number(&record, "totalSamplesReceived");
        }
    }
    Ok(samples)
}

fn string(record: &JsValue, key: &str) -> Option<String> {
    Reflect::get(record, &key.into()).ok()?.as_string()
}

fn number(record: &JsValue, key: &str) -> f64 {
    Reflect::get(record, &key.into()).ok().and_then(|value| value.as_f64()).unwrap_or(0.0)
}

fn stats_error(error: JsValue) -> ClientError {
    ClientError::Transport(format!("receiver stats: {error:?}"))
}
