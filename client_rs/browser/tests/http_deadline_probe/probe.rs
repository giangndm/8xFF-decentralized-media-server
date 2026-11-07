use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub async fn probe(url: String) -> bool {
    atm0s_media_client_browser::request_protobuf_with_timeout(&url, None, b"probe", 150).await.is_err()
}
#[wasm_bindgen]
pub fn cancel(url: String) -> bool {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    let mut request = Box::pin(atm0s_media_client_browser::request_protobuf_with_timeout(&url, None, b"probe", 150));
    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoopWake));
    let pending = matches!(request.as_mut().poll(&mut Context::from_waker(&waker)), Poll::Pending);
    drop(request);
    pending
}
