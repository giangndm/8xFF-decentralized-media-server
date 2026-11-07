use super::EventSink;
use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::{closure::Closure, prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcConfiguration, RtcDataChannel, RtcDataChannelInit, RtcPeerConnection, RtcRtpTransceiver, RtcSdpType, RtcSessionDescriptionInit};

pub struct BrowserPeer {
    peer: RtcPeerConnection,
    control: RtcDataChannel,
    listeners: Vec<Closure<dyn FnMut(JsValue)>>,
}

impl BrowserPeer {
    pub fn new(config: Option<RtcConfiguration>) -> Result<Self, JsValue> {
        let peer = match config {
            Some(config) => RtcPeerConnection::new_with_configuration(&config)?,
            None => RtcPeerConnection::new()?,
        };
        let init = RtcDataChannelInit::new();
        init.set_negotiated(true);
        init.set_id(0);
        let control = peer.create_data_channel_with_data_channel_dict("data", &init);
        control.set_binary_type(web_sys::RtcDataChannelType::Arraybuffer);
        Ok(Self { peer, control, listeners: Vec::new() })
    }

    pub fn raw(&self) -> &RtcPeerConnection {
        &self.peer
    }
    pub fn control(&self) -> &RtcDataChannel {
        &self.control
    }
    pub fn control_open(&self) -> bool {
        self.control.ready_state() == web_sys::RtcDataChannelState::Open
    }

    pub fn add_transceiver(&self, kind: &str, direction: web_sys::RtcRtpTransceiverDirection, simulcast: bool) -> Result<RtcRtpTransceiver, JsValue> {
        let init = web_sys::RtcRtpTransceiverInit::new();
        init.set_direction(direction);
        if simulcast && direction == web_sys::RtcRtpTransceiverDirection::Sendonly {
            let encodings = Array::new();
            for rid in ["0", "1", "2"] {
                let encoding = js_sys::Object::new();
                Reflect::set(&encoding, &"rid".into(), &rid.into())?;
                Reflect::set(&encoding, &"active".into(), &true.into())?;
                encodings.push(&encoding);
            }
            init.set_send_encodings(&encodings.into());
        }
        Ok(self.peer.add_transceiver_with_str_and_init(kind, &init))
    }

    pub async fn create_offer(&self) -> Result<String, JsValue> {
        let offer = JsFuture::from(self.peer.create_offer()).await?;
        Reflect::get(&offer, &"sdp".into())?.as_string().ok_or_else(|| JsValue::from_str("offer has no SDP"))
    }

    pub async fn set_local_description(&self, sdp: &str) -> Result<(), JsValue> {
        let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        desc.set_sdp(sdp);
        JsFuture::from(self.peer.set_local_description(&desc)).await.map(|_| ())
    }

    pub async fn set_remote_answer(&self, sdp: &str) -> Result<(), JsValue> {
        let desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.set_sdp(sdp);
        JsFuture::from(self.peer.set_remote_description(&desc)).await.map(|_| ())
    }

    pub async fn restart_ice_offer(&self) -> Result<String, JsValue> {
        let options = web_sys::RtcOfferOptions::new();
        options.set_ice_restart(true);
        let offer = JsFuture::from(self.peer.create_offer_with_rtc_offer_options(&options)).await?;
        Reflect::get(&offer, &"sdp".into())?.as_string().ok_or_else(|| JsValue::from_str("restart offer has no SDP"))
    }

    pub fn send_control(&self, bytes: &[u8]) -> Result<(), JsValue> {
        self.control.send_with_u8_array(bytes)
    }
    pub async fn replace_track(&self, transceiver: &RtcRtpTransceiver, track: Option<&web_sys::MediaStreamTrack>) -> Result<(), JsValue> {
        JsFuture::from(transceiver.sender().replace_track(track)).await.map(|_| ())
    }

    pub fn install_event_sink(&mut self, sink: EventSink) -> Result<(), JsValue> {
        self.clear_listeners();
        let sink_ice = sink.clone();
        let ice = Closure::wrap(Box::new(move |event: JsValue| {
            if let Ok(candidate) = Reflect::get(&event, &"candidate".into()) {
                if !candidate.is_null() && !candidate.is_undefined() {
                    if let Ok(value) = Reflect::get(&candidate, &"candidate".into()) {
                        let obj = js_sys::Object::new();
                        let _ = Reflect::set(&obj, &"type".into(), &"ice_candidate".into());
                        let _ = Reflect::set(&obj, &"candidate".into(), &value);
                        let _ = sink_ice.call1(&JsValue::UNDEFINED, &obj);
                    }
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
        self.peer.set_onicecandidate(Some(ice.as_ref().unchecked_ref()));
        self.listeners.push(ice);

        let sink_track = sink.clone();
        let track = Closure::wrap(Box::new(move |event: JsValue| {
            let obj = js_sys::Object::new();
            let _ = Reflect::set(&obj, &"type".into(), &"track".into());
            if let Ok(value) = Reflect::get(&event, &"track".into()) {
                let _ = Reflect::set(&obj, &"track".into(), &value);
            }
            if let Ok(value) = Reflect::get(&event, &"streams".into()) {
                let _ = Reflect::set(&obj, &"streams".into(), &value);
            }
            if let Ok(value) = Reflect::get(&event, &"transceiver".into()) {
                let _ = Reflect::set(&obj, &"transceiver".into(), &value);
            }
            let _ = sink_track.call1(&JsValue::UNDEFINED, &obj);
        }) as Box<dyn FnMut(JsValue)>);
        self.peer.set_ontrack(Some(track.as_ref().unchecked_ref()));
        self.listeners.push(track);

        let sink_conn = sink.clone();
        let peer_state = self.peer.clone();
        let state = Closure::wrap(Box::new(move |_event: JsValue| {
            let obj = js_sys::Object::new();
            let _ = Reflect::set(&obj, &"type".into(), &"connection_state".into());
            let state = Reflect::get(&peer_state, &"connectionState".into())
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_else(|| "unknown".into());
            let _ = Reflect::set(&obj, &"state".into(), &state.into());
            let _ = sink_conn.call1(&JsValue::UNDEFINED, &obj);
        }) as Box<dyn FnMut(JsValue)>);
        self.peer.set_onconnectionstatechange(Some(state.as_ref().unchecked_ref()));
        self.listeners.push(state);

        let open_sink = sink.clone();
        let open = Closure::wrap(Box::new(move |_event: JsValue| {
            let _ = open_sink.call1(&JsValue::UNDEFINED, &"data_open".into());
        }) as Box<dyn FnMut(JsValue)>);
        self.control.set_onopen(Some(open.as_ref().unchecked_ref()));
        self.listeners.push(open);

        let msg_sink = sink;
        let msg = Closure::wrap(Box::new(move |event: JsValue| {
            if let Ok(data) = Reflect::get(&event, &"data".into()) {
                let bytes = Uint8Array::new(&data);
                let obj = js_sys::Object::new();
                let _ = Reflect::set(&obj, &"type".into(), &"data_message".into());
                let _ = Reflect::set(&obj, &"bytes".into(), &bytes);
                let _ = msg_sink.call1(&JsValue::UNDEFINED, &obj);
            }
        }) as Box<dyn FnMut(JsValue)>);
        self.control.set_onmessage(Some(msg.as_ref().unchecked_ref()));
        self.listeners.push(msg);
        Ok(())
    }

    pub fn close(&mut self) {
        self.clear_listeners();
        self.peer.close();
    }
    fn clear_listeners(&mut self) {
        self.peer.set_onicecandidate(None);
        self.peer.set_ontrack(None);
        self.peer.set_onconnectionstatechange(None);
        self.control.set_onopen(None);
        self.control.set_onmessage(None);
        self.listeners.clear();
    }
}

impl Drop for BrowserPeer {
    fn drop(&mut self) {
        self.clear_listeners();
        self.peer.close();
    }
}
