use crate::{
    media::{BrowserStream, BrowserTrack},
    peer::{BrowserWebRtcPeer, EventSink},
};
use crate::{BrowserClient, BrowserHttpClient};
use atm0s_media_client_core::{Atm0sClient, Client, ClientConfig, ClientError, Input};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use wasm_bindgen::{closure::Closure, prelude::*};
use wasm_bindgen_futures::spawn_local;
use web_sys::{MediaStream, MediaStreamTrack, RtcConfiguration, RtcRtpTransceiver};

pub struct BrowserRuntime {
    pub core: BrowserClient,
    pump_timer: Option<i32>,
    pump_callback: Option<Closure<dyn FnMut()>>,
    draining: bool,
    events: VecDeque<atm0s_media_client_core::ClientEvent>,
}

impl BrowserRuntime {
    pub fn new(config: ClientConfig, rtc: Option<RtcConfiguration>) -> Result<Self, JsValue> {
        let http = BrowserHttpClient::new("").with_timeout(config.rpc_timeout_ms);
        Ok(Self {
            core: Atm0sClient::new(Client::new(config), http, BrowserWebRtcPeer::new(rtc)?),
            pump_timer: None,
            pump_callback: None,
            draining: false,
            events: VecDeque::new(),
        })
    }
    pub fn register_track(&mut self, id: impl Into<String>, track: MediaStreamTrack) {
        let _ = self.core.register_track(id, BrowserTrack(track));
    }
    pub fn register_stream(&mut self, name: impl Into<String>, stream: MediaStream) {
        let _ = self.core.register_stream(name, BrowserStream(stream));
    }
    pub fn stream(&self, name: &str) -> Option<BrowserStream> {
        self.core.peer.stream(name).cloned()
    }
    pub fn transceiver(&self, name: &str) -> Option<&RtcRtpTransceiver> {
        self.core.peer.transceiver(name)
    }
    pub fn install_event_sink(&mut self, sink: EventSink) -> Result<(), JsValue> {
        self.core.peer.install_event_sink(sink)
    }
    pub fn prepare_transceiver(&mut self, name: impl Into<String>, kind: atm0s_media_client_core::Kind, direction: atm0s_media_client_core::Direction, simulcast: bool) -> Result<(), JsValue> {
        self.core.peer.prepare_transceiver(name, kind, direction, simulcast)
    }
    pub async fn attach_track(&self, name: &str, source_id: &str) -> Result<(), JsValue> {
        let transceiver = self.core.peer.transceiver(name).cloned().ok_or_else(|| JsValue::from_str("unknown transceiver"))?;
        let track = self.core.peer.track(source_id).ok_or_else(|| JsValue::from_str("unknown track"))?.0.clone();
        self.core.peer.peer().replace_track(&transceiver, Some(&track)).await
    }
    pub fn pump_browser_events(&mut self, now: u64) {
        self.core.on_input(now, Input::Tick);
    }
    pub async fn drain(&mut self) -> Result<Vec<atm0s_media_client_core::ClientEvent>, JsValue> {
        let now = web_sys::window().and_then(|w| w.performance()).map(|p| p.now() as u64).unwrap_or(0);
        self.drain_at(now).await
    }
    pub async fn drain_at(&mut self, _now: u64) -> Result<Vec<atm0s_media_client_core::ClientEvent>, JsValue> {
        if self.draining {
            return Ok(Vec::new());
        }
        self.draining = true;
        let pump_result = self.core.pump().await.map_err(|error| JsValue::from_str(&format!("client: {error:?}")));
        self.draining = false;
        pump_result?;
        let mut events = Vec::new();
        while let Some(event) = self.core.poll_event() {
            events.push(event);
        }
        Ok(events)
    }
    // Only one task may drive this local runtime. Other timer invocations use
    // try_borrow_mut and skip it while I/O is pending; WebRTC callbacks write to
    // their independent queue and never borrow the runtime.
    #[allow(clippy::await_holding_refcell_ref)]
    pub fn start_pump(shared: Rc<RefCell<Self>>, interval_ms: i32) -> Result<i32, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        shared.borrow_mut().stop_pump();
        let tick_weak = Rc::downgrade(&shared);
        let callback = Closure::<dyn FnMut()>::new(move || {
            let Some(tick_shared) = tick_weak.upgrade() else { return };
            let now = web_sys::window().and_then(|w| w.performance()).map(|p| p.now() as u64).unwrap_or(0);
            let Ok(mut runtime) = tick_shared.try_borrow_mut() else {
                return;
            };
            runtime.pump_browser_events(now);
            if runtime.draining {
                return;
            }
            let task_shared = tick_shared.clone();
            drop(runtime);
            spawn_local(async move {
                let Ok(mut runtime) = task_shared.try_borrow_mut() else { return };
                if let Ok(events) = runtime.drain_at(now).await {
                    runtime.events.extend(events);
                }
            });
        });
        let id = window.set_interval_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), interval_ms)?;
        let mut runtime = shared.borrow_mut();
        runtime.pump_timer = Some(id);
        runtime.pump_callback = Some(callback);
        Ok(id)
    }
    pub fn stop_pump(&mut self) {
        if let Some(id) = self.pump_timer.take() {
            if let Some(window) = web_sys::window() {
                window.clear_interval_with_handle(id);
            }
        }
        self.pump_callback.take();
    }
    pub fn now_ms() -> u64 {
        web_sys::window().and_then(|w| w.performance()).map(|p| p.now() as u64).unwrap_or(0)
    }
    pub fn poll_event(&mut self) -> Option<atm0s_media_client_core::ClientEvent> {
        self.events.pop_front()
    }
    pub fn take_events(&mut self) -> Vec<atm0s_media_client_core::ClientEvent> {
        self.events.drain(..).collect()
    }

    pub async fn command(&mut self, command: atm0s_media_client_core::Command) -> Result<atm0s_media_client_core::OperationResult, ClientError> {
        let operation = self.core.command(Self::now_ms(), command)?;
        loop {
            let now = Self::now_ms();
            self.pump_browser_events(now);
            let produced = self.drain_at(now).await.map_err(|e| ClientError::Transport(format!("browser: {:?}", e)))?;
            let mut completion = None;
            for event in produced {
                if let atm0s_media_client_core::ClientEvent::OperationCompleted { id, result } = event {
                    if id == operation {
                        completion = Some(result);
                    } else {
                        self.events.push_back(atm0s_media_client_core::ClientEvent::OperationCompleted { id, result });
                    }
                } else {
                    self.events.push_back(event);
                }
            }
            if let Some(result) = completion {
                return result;
            }
            if let Some(deadline) = self.core.core.next_deadline() {
                if deadline <= now {
                    self.core.on_input(now, Input::Tick);
                    continue;
                }
            }
            sleep_ms(10).await?;
        }
    }
}

async fn sleep_ms(milliseconds: i32) -> Result<(), ClientError> {
    let window = web_sys::window().ok_or_else(|| ClientError::Transport("window unavailable".into()))?;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), milliseconds);
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|e| ClientError::Transport(format!("timer: {:?}", e)))
}

impl Drop for BrowserRuntime {
    fn drop(&mut self) {
        self.stop_pump();
    }
}
