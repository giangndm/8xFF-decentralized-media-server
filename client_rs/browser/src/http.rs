use atm0s_media_client_core::{ClientError, HttpClient, HttpRequest, HttpResponse};
use js_sys::Uint8Array;
use std::future::Future;
use std::pin::Pin;
use wasm_bindgen::{closure::Closure, prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;

/// Raw Protobuf-over-HTTP. The server uses application/grpc as a content type
/// but does not use gRPC framing.
pub async fn post_protobuf(url: &str, token: Option<&str>, body: &[u8]) -> Result<Vec<u8>, JsValue> {
    let response = request_protobuf(url, token, body).await?;
    if response.status < 200 || response.status >= 300 {
        return Err(JsValue::from_str(&format!("HTTP {}", response.status)));
    }
    Ok(response.body)
}

/// Fetch a protobuf response while preserving the HTTP status. Generic core
/// clients need the status to classify non-2xx responses without losing the
/// server's diagnostic body.
pub async fn request_protobuf(url: &str, token: Option<&str>, body: &[u8]) -> Result<HttpResponse, JsValue> {
    request_protobuf_with_timeout(url, token, body, 5_000).await
}

/// Apply one deadline to both response headers and the complete response body.
pub async fn request_protobuf_with_timeout(url: &str, token: Option<&str>, body: &[u8], timeout_ms: u64) -> Result<HttpResponse, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let deadline = RequestDeadline::new(&window, timeout_ms)?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_signal(Some(&deadline.controller.signal()));
    let body_js: JsValue = Uint8Array::from(body).into();
    init.set_body(&body_js);
    let headers = Headers::new()?;
    headers.set("Content-Type", "application/grpc")?;
    if let Some(token) = token {
        headers.set("Authorization", &format!("Bearer {token}"))?;
    }
    init.set_headers(&headers);
    let request = web_sys::Request::new_with_str_and_init(url, &init)?;
    let response = JsFuture::from(window.fetch_with_request(&request)).await?.dyn_into::<web_sys::Response>()?;
    let bytes = JsFuture::from(response.array_buffer()?).await?;
    Ok(HttpResponse {
        status: response.status(),
        body: Uint8Array::new(&bytes).to_vec(),
    })
}

/// Typed browser HTTP adapter used by generic client facades. It intentionally
/// exposes bytes: Protobuf encoding/decoding stays in the protocol/core crate.
#[derive(Clone, Debug)]
pub struct BrowserHttpClient {
    pub base_url: String,
    pub timeout_ms: u64,
}

impl BrowserHttpClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            timeout_ms: 5_000,
        }
    }
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
    pub async fn post(&self, path: &str, token: Option<&str>, body: &[u8]) -> Result<Vec<u8>, JsValue> {
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            format!(
                "{}{}",
                self.base_url,
                if path.starts_with('/') {
                    path.to_owned()
                } else {
                    format!("/{path}")
                }
            )
        };
        let response = request_protobuf_with_timeout(&url, token, body, self.timeout_ms).await?;
        if !(200..300).contains(&response.status) {
            return Err(JsValue::from_str(&format!("HTTP {}", response.status)));
        }
        Ok(response.body)
    }
}

impl HttpClient for BrowserHttpClient {
    fn request<'a>(&'a mut self, request: HttpRequest) -> Pin<Box<dyn Future<Output = Result<HttpResponse, ClientError>> + 'a>> {
        Box::pin(async move {
            let token = request
                .headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("authorization"))
                .map(|(_, value)| value.trim_start_matches("Bearer "));
            let response = request_protobuf_with_timeout(&request.url, token, &request.body, self.timeout_ms)
                .await
                .map_err(|error| ClientError::Transport(format!("http: {:?}", error)))?;
            Ok(response)
        })
    }
}

/// The future owns this guard across both awaits. Dropping an unfinished request
/// cancels its network activity and releases its timer and callback as well.
struct RequestDeadline {
    window: web_sys::Window,
    controller: web_sys::AbortController,
    timer: i32,
    _callback: Closure<dyn FnMut()>,
}

impl RequestDeadline {
    fn new(window: &web_sys::Window, timeout_ms: u64) -> Result<Self, JsValue> {
        let controller = web_sys::AbortController::new()?;
        let abort = controller.clone();
        let callback = Closure::<dyn FnMut()>::new(move || abort.abort());
        let timer = window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), timeout_ms.min(i32::MAX as u64) as i32)?;
        Ok(Self {
            window: window.clone(),
            controller,
            timer,
            _callback: callback,
        })
    }
}

impl Drop for RequestDeadline {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.timer);
        self.controller.abort();
    }
}
