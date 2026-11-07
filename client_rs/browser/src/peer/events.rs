use super::*;
use atm0s_media_client_core::{RtcEvent, RtcEventEnvelope};
use js_sys::Uint8Array;

impl BrowserWebRtcPeer {
    pub(super) fn next_input(&mut self) -> Option<Input> {
        let (generation, event) = loop {
            let queued = self.input_events.borrow_mut().pop_front()?;
            if queued.0 == self.callback_generation.get() {
                break queued;
            }
        };
        if event.as_string().as_deref() == Some("data_open") {
            return Some(Input::Rtc(RtcEventEnvelope {
                generation,
                event: RtcEvent::DataChannelOpen,
            }));
        }
        let kind = Reflect::get(&event, &"type".into()).ok().and_then(|value| value.as_string()).unwrap_or_default();
        let rtc_event = match kind.as_str() {
            "data_message" => Reflect::get(&event, &"bytes".into()).ok().map(|bytes| RtcEvent::Data(Uint8Array::new(&bytes).to_vec())),
            "ice_candidate" => Reflect::get(&event, &"candidate".into()).ok().and_then(|value| value.as_string()).map(RtcEvent::LocalCandidate),
            "connection_state" => Reflect::get(&event, &"state".into()).ok().and_then(|value| value.as_string()).map(RtcEvent::ConnectionState),
            "track" => {
                let track = Reflect::get(&event, &"track".into()).ok()?.dyn_into::<web_sys::MediaStreamTrack>().ok()?;
                let id = track.id();
                let transceiver = Reflect::get(&event, &"transceiver".into()).ok()?.dyn_into::<RtcRtpTransceiver>().ok()?;
                let name = self.transceiver_name(&transceiver)?;
                if let Ok(streams) = Reflect::get(&event, &"streams".into()) {
                    let first = js_sys::Array::from(&streams).get(0);
                    if let Ok(stream) = first.dyn_into::<web_sys::MediaStream>() {
                        self.streams.insert(name.clone(), BrowserStream(stream));
                    } else if let Ok(stream) = web_sys::MediaStream::new() {
                        stream.add_track(&track);
                        self.streams.insert(name.clone(), BrowserStream(stream));
                    }
                } else if let Ok(stream) = web_sys::MediaStream::new() {
                    stream.add_track(&track);
                    self.streams.insert(name.clone(), BrowserStream(stream));
                }
                self.tracks.insert(id.clone(), BrowserTrack(track));
                Some(RtcEvent::TrackReady { name, track_id: id })
            }
            _ => None,
        }?;
        Some(Input::Rtc(RtcEventEnvelope { generation, event: rtc_event }))
    }
}
