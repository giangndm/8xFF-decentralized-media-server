//! Page controls own session lifetime; timers hold weak references only.
use crate::{
    harness::{append_log, input_value, set_assert},
    scenarios::Scenario,
    session::{ConnectForm, Session},
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::Document;

#[derive(Default)]
struct App {
    session: Option<Rc<RefCell<Option<Session>>>>,
    timer: Option<Timer>,
    connecting: bool,
    listeners: Vec<Closure<dyn FnMut()>>,
}
thread_local! { static APP: RefCell<App> = RefCell::new(App::default()); }

pub fn install(document: Document) -> Result<(), JsValue> {
    if let Some(value) = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|query| query.strip_prefix("?example=").map(str::to_owned))
    {
        if let Some(select) = document.get_element_by_id("scenario").and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok()) {
            select.set_value(&value);
        }
    }
    for (id, action) in [("connect", connect as fn(Document)), ("disconnect", disconnect as fn(Document))] {
        let element = document.get_element_by_id(id).ok_or_else(|| JsValue::from_str("missing page control"))?;
        let document = document.clone();
        let callback = Closure::new(move || action(document.clone()));
        element.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
        APP.with(|app| app.borrow_mut().listeners.push(callback));
    }
    if let Some(root) = document.document_element() {
        root.set_attribute("data-client-ready", "true")?;
    }
    Ok(())
}

fn connect(document: Document) {
    let accepted = APP.with(|app| {
        let mut app = app.borrow_mut();
        if app.connecting || app.session.is_some() {
            false
        } else {
            app.connecting = true;
            true
        }
    });
    if !accepted {
        return;
    }
    button_enabled(&document, "connect", false);
    for name in ["control", "scenario", "media", "cleanup"] {
        set_assert(&document, name, "pending");
    }
    spawn_local(async move {
        let result = match read_form(&document) {
            Ok(form) => Session::connect(document.clone(), form).await,
            Err(error) => Err(error),
        };
        APP.with(|app| app.borrow_mut().connecting = false);
        match result {
            Ok(session) => {
                let shared = Rc::new(RefCell::new(Some(session)));
                match Timer::start(&shared) {
                    Ok(timer) => APP.with(|app| {
                        let mut app = app.borrow_mut();
                        app.session = Some(shared);
                        app.timer = Some(timer);
                    }),
                    Err(error) => {
                        log_error(&document, error);
                        button_enabled(&document, "connect", true);
                    }
                }
            }
            Err(error) => {
                log_error(&document, error);
                button_enabled(&document, "connect", true);
            }
        }
    });
}

fn disconnect(document: Document) {
    let shared = APP.with(|app| {
        let mut app = app.borrow_mut();
        app.timer.take();
        app.session.take()
    });
    let Some(shared) = shared else { return };
    spawn_local(async move {
        let mut session = loop {
            if let Some(session) = shared.borrow_mut().take() {
                break session;
            }
            pause().await;
        };
        if let Err(error) = session.disconnect().await {
            log_error(&document, error);
        }
        button_enabled(&document, "connect", true);
    });
}

fn read_form(document: &Document) -> Result<ConnectForm, JsValue> {
    let input = |id| document.get_element_by_id(id).map(|element| input_value(&element)).ok_or_else(|| JsValue::from_str("missing input"));
    let scenario = document
        .get_element_by_id("scenario")
        .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .and_then(|select| Scenario::parse(&select.value()))
        .ok_or_else(|| JsValue::from_str("unknown scenario"))?;
    Ok(ConnectForm {
        gateway: input("gateway")?,
        room: input("room")?,
        peer: input("peer")?,
        token: input("token")?,
        scenario,
    })
}
fn button_enabled(document: &Document, id: &str, enabled: bool) {
    if let Some(button) = document.get_element_by_id(id) {
        if enabled {
            let _ = button.remove_attribute("disabled");
        } else {
            let _ = button.set_attribute("disabled", "");
        }
    }
}
fn log_error(document: &Document, error: JsValue) {
    if let Some(log) = document.get_element_by_id("event-log") {
        append_log(&log, &format!("error: {error:?}"));
    }
    web_sys::console::error_1(&error);
}
struct Timer {
    id: i32,
    _callback: Closure<dyn FnMut()>,
}
impl Timer {
    fn start(shared: &Rc<RefCell<Option<Session>>>) -> Result<Self, JsValue> {
        let weak = Rc::downgrade(shared);
        let callback = Closure::new(move || {
            let Some(shared) = weak.upgrade() else { return };
            spawn_local(async move {
                let Some(mut session) = shared.borrow_mut().take() else { return };
                if let Err(error) = session.tick().await {
                    session.log(&format!("error: {error:?}"));
                    web_sys::console::error_1(&error);
                }
                *shared.borrow_mut() = Some(session);
            });
        });
        let id = web_sys::window()
            .ok_or_else(|| JsValue::from_str("window unavailable"))?
            .set_interval_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), 50)?;
        Ok(Self { id, _callback: callback })
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(self.id);
        }
    }
}
async fn pause() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 10);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
