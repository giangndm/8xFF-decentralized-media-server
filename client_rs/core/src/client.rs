use super::*;
pub(crate) struct Pending {
    pub(crate) operation: OperationId,
    pub(crate) generation: Generation,
    pub(crate) deadline: Timestamp,
    pub(crate) await_rpc: bool,
}

pub struct Client {
    pub(crate) cfg: ClientConfig,
    pub(crate) now: Timestamp,
    pub(crate) next_operation: OperationId,
    pub(crate) next_effect: EffectId,
    pub(crate) generation: Generation,
    pub(crate) session: SessionState,
    pub(crate) conn_id: Option<String>,
    pub(crate) ice_lite: bool,
    pub(crate) ice_ready: bool,
    pub(crate) ice_candidates: Vec<String>,
    pub(crate) token: Option<String>,
    pub(crate) gateway: Option<String>,
    pub(crate) room: Option<RoomInfo>,
    pub(crate) join_config: Option<JoinConfig>,
    pub(crate) connect_pending: Option<OperationId>,
    pub(crate) connect_version: String,
    pub(crate) connect_preparation: VecDeque<Effect>,
    pub(crate) connect_remote_ready: bool,
    pub(crate) connect_channel_ready: bool,
    pub(crate) restart_pending: Option<OperationId>,
    pub(crate) pending_local_sdp: Option<String>,
    pub(crate) pending_remote_sdp: Option<String>,
    pub(crate) negotiation_pending: Option<OperationId>,
    pub(crate) senders: Vec<SenderState>,
    pub(crate) receivers: Vec<ReceiverState>,
    pub(crate) channels: HashMap<String, bool>,
    pub(crate) operations: HashMap<OperationId, Timestamp>,
    pub(crate) pending: HashMap<EffectId, Pending>,
    pub(crate) active_effects: HashMap<EffectId, Effect>,
    pub(crate) deferred_sender: HashMap<EffectId, (OperationId, session::Request)>,
    pub(crate) rpc_effects: HashMap<u32, (EffectId, ResponseKind)>,
    pub(crate) rpc_followups: HashMap<OperationId, session::Request>,
    pub(crate) outputs: VecDeque<Output>,
    pub(crate) rpc_seq: u32,
    pub(crate) rpc_req: u32,
}

impl Client {
    pub fn new(cfg: ClientConfig) -> Self {
        Self {
            cfg,
            now: 0,
            next_operation: 1,
            next_effect: 1,
            generation: 0,
            session: SessionState::New,
            conn_id: None,
            ice_lite: false,
            ice_ready: false,
            ice_candidates: Vec::new(),
            token: None,
            gateway: None,
            room: None,
            join_config: None,
            connect_pending: None,
            connect_version: String::new(),
            connect_preparation: VecDeque::new(),
            connect_remote_ready: false,
            connect_channel_ready: false,
            restart_pending: None,
            pending_local_sdp: None,
            pending_remote_sdp: None,
            negotiation_pending: None,
            senders: vec![],
            receivers: vec![],
            channels: HashMap::new(),
            operations: HashMap::new(),
            pending: HashMap::new(),
            active_effects: HashMap::new(),
            deferred_sender: HashMap::new(),
            rpc_effects: HashMap::new(),
            rpc_followups: HashMap::new(),
            outputs: VecDeque::new(),
            rpc_seq: 0,
            rpc_req: 0,
        }
    }
    pub fn snapshot(&self) -> ClientSnapshot {
        ClientSnapshot {
            session: self.session,
            generation: self.generation,
            conn_id: self.conn_id.clone(),
            ice_lite: self.ice_lite,
            room: self.room.clone(),
            senders: self.senders.clone(),
            receivers: self.receivers.clone(),
            channels: self.channels.keys().cloned().collect(),
        }
    }
    pub fn poll_output(&mut self) -> Option<Output> {
        self.outputs.pop_front()
    }
    pub fn next_deadline(&self) -> Option<Timestamp> {
        self.pending.values().map(|p| p.deadline).chain(self.operations.values().copied()).min()
    }
    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub(crate) fn advance_connect(&mut self, operation: OperationId) {
        if self.connect_pending != Some(operation) {
            return;
        }
        if let Some(effect) = self.connect_preparation.pop_front() {
            self.effect(operation, effect);
        } else if let Some(offer) = self.pending_local_sdp.clone() {
            self.send_connection_request(operation, &offer);
        } else {
            self.effect(
                operation,
                Effect::CreateOffer {
                    generation: self.generation,
                    restart: false,
                },
            );
        }
    }
    pub(crate) fn send_connection_request(&mut self, operation: OperationId, offer: &str) {
        let request = gateway_request(&self.connect_version, self.join_config.as_ref(), &self.senders, &self.receivers, offer);
        let path = if self.restart_pending == Some(operation) {
            format!("{}/restart-ice", self.conn_id.as_deref().unwrap_or_default())
        } else {
            "connect".to_owned()
        };
        self.effect(
            operation,
            Effect::Http(HttpRequest {
                method: HttpMethod::Post,
                url: format!("{}/webrtc/{path}", self.gateway.as_deref().unwrap_or_default()),
                headers: vec![
                    ("Authorization".into(), format!("Bearer {}", self.token.as_deref().unwrap_or_default())),
                    ("Content-Type".into(), "application/grpc".into()),
                ],
                body: request.encode_to_vec(),
            }),
        );
    }

    pub(crate) fn finish_connect(&mut self) {
        if self.connect_remote_ready && self.connect_channel_ready {
            if let Some(operation) = self.connect_pending {
                self.session = SessionState::Connected;
                self.event(ClientEvent::SessionChanged(self.session));
                self.complete(operation, Ok(OperationResult::Connected));
            }
        }
    }

    pub(crate) fn local_candidate(&mut self, candidate: String) {
        if !self.ice_ready {
            self.ice_candidates.push(candidate);
            return;
        }
        if self.ice_lite {
            return;
        }
        if let (Some(gateway), Some(conn_id)) = (self.gateway.clone(), self.conn_id.clone()) {
            let body = gateway::RemoteIceRequest { candidates: vec![candidate] };
            self.effect(
                0,
                Effect::Http(HttpRequest {
                    method: HttpMethod::Post,
                    url: format!("{gateway}/webrtc/{conn_id}/ice-candidate"),
                    headers: vec![("Content-Type".into(), "application/grpc".into())],
                    body: body.encode_to_vec(),
                }),
            );
        }
    }

    pub(crate) fn effect_ok(&mut self, op: OperationId, result: EffectResult) {
        match result {
            EffectResult::Conn { conn_id, sdp, ice_lite } => {
                self.conn_id = Some(conn_id);
                self.ice_lite = ice_lite;
                self.ice_ready = true;
                let candidates = std::mem::take(&mut self.ice_candidates);
                for candidate in candidates {
                    self.local_candidate(candidate);
                }
                self.pending_remote_sdp = Some(sdp);
                self.effect(
                    op,
                    Effect::SetLocalDescription {
                        generation: self.generation,
                        sdp: self.pending_local_sdp.clone().unwrap_or_default(),
                    },
                );
            }
            EffectResult::Sdp(sdp) if self.connect_pending == Some(op) => {
                self.pending_local_sdp = Some(sdp);
                self.advance_connect(op);
            }
            EffectResult::Sdp(sdp) if self.restart_pending == Some(op) => {
                self.pending_local_sdp = Some(sdp.clone());
                self.send_connection_request(op, &sdp);
            }
            EffectResult::Sdp(sdp) if self.session == SessionState::Connected => {
                self.negotiation_pending = Some(op);
                self.pending_local_sdp = Some(sdp.clone());
                self.effect(op, Effect::SetLocalDescription { generation: self.generation, sdp });
            }
            EffectResult::DataChannelOpen => {}
            EffectResult::TrackReady { name, track_id } => self.event(ClientEvent::ReceiverReady { name, track_id }),
            _ => {
                if op != 0 && self.connect_pending != Some(op) {
                    self.complete(op, Ok(OperationResult::Updated))
                }
            }
        }
    }
    pub(crate) fn on_data(&mut self, bytes: Vec<u8>) {
        let Ok(ev) = session::ServerEvent::decode(bytes.as_slice()) else {
            self.event(ClientEvent::Error(ClientError::Protocol("invalid server event".into())));
            return;
        };
        let Some(event) = ev.event else { return };
        match event {
            session::server_event::Event::Room(room) => {
                if let Some(e) = room.event {
                    self.on_room_event(e);
                }
            }
            session::server_event::Event::MessageChannel(ch) => {
                if let Some(session::server_event::message_channel::Event::Message(m)) = ch.event {
                    self.event(ClientEvent::Message {
                        label: ch.label,
                        peer: m.peer,
                        data: m.message,
                    });
                }
            }
            session::server_event::Event::Features(features) => {
                use media_server_protocol::protobuf::features::{mixer, server_event};
                if let Some(server_event::Event::Mixer(event)) = features.event {
                    match event.event {
                        Some(mixer::server_event::Event::SlotSet(slot)) => self.event(ClientEvent::Mixer(MixerEvent::OutputChanged {
                            slot: slot.slot,
                            source: slot.source.map(|source| Source {
                                peer: source.peer,
                                track: source.track,
                            }),
                        })),
                        Some(mixer::server_event::Event::SlotUnset(slot)) => self.event(ClientEvent::Mixer(MixerEvent::OutputChanged { slot: slot.slot, source: None })),
                        None => {}
                    }
                }
            }
            session::server_event::Event::Response(r) => self.resolve_rpc(r),
            _ => {}
        }
    }
    pub(crate) fn on_room_event(&mut self, e: session::server_event::room::Event) {
        use session::server_event::room::Event;
        match e {
            Event::PeerJoined(x) => self.event(ClientEvent::Room(RoomEvent::PeerJoined { peer: x.peer, metadata: x.metadata })),
            Event::PeerUpdated(x) => self.event(ClientEvent::Room(RoomEvent::PeerUpdated { peer: x.peer, metadata: x.metadata })),
            Event::PeerLeaved(x) => self.event(ClientEvent::Room(RoomEvent::PeerLeft { peer: x.peer })),
            Event::TrackStarted(x) => self.event(ClientEvent::Room(RoomEvent::TrackStarted {
                source: Source { peer: x.peer, track: x.track },
                kind: if x.kind == 0 {
                    Kind::Audio
                } else {
                    Kind::Video
                },
                metadata: x.metadata,
            })),
            Event::TrackUpdated(x) => self.event(ClientEvent::Room(RoomEvent::TrackUpdated {
                source: Source { peer: x.peer, track: x.track },
                kind: if x.kind == 0 {
                    Kind::Audio
                } else {
                    Kind::Video
                },
                metadata: x.metadata,
            })),
            Event::TrackStopped(x) => self.event(ClientEvent::Room(RoomEvent::TrackStopped {
                source: Source { peer: x.peer, track: x.track },
                kind: if x.kind == 0 {
                    Kind::Audio
                } else {
                    Kind::Video
                },
            })),
        }
    }
    pub(crate) fn resolve_rpc(&mut self, r: session::Response) {
        let Some((effect_id, expected)) = self.rpc_effects.remove(&r.req_id) else { return };
        let Some(p) = self.pending.remove(&effect_id) else { return };
        self.active_effects.remove(&effect_id);
        let Some(response) = r.response else {
            self.complete(p.operation, Err(ClientError::Protocol("empty server RPC response".into())));
            return;
        };
        if let session::response::Response::Error(err) = response {
            self.complete(p.operation, Err(ClientError::Protocol(format!("server error {}: {}", err.code, err.message))));
        } else if !response_matches(expected, &response) {
            self.complete(p.operation, Err(ClientError::Protocol("unexpected server RPC response variant".into())));
        } else if let Some(request) = self.rpc_followups.remove(&p.operation) {
            self.send_rpc(p.operation, request, expected);
        } else if self.negotiation_pending == Some(p.operation) {
            match response {
                session::response::Response::Session(session::response::Session {
                    response: Some(session::response::session::Response::Sdp(answer)),
                }) => {
                    self.pending_remote_sdp = Some(answer.sdp.clone());
                    self.effect(
                        p.operation,
                        Effect::SetRemoteDescription {
                            generation: self.generation,
                            sdp: answer.sdp,
                        },
                    );
                }
                _ => self.complete(p.operation, Err(ClientError::Protocol("expected SDP negotiation response".into()))),
            }
        } else {
            self.complete(p.operation, Ok(OperationResult::Updated));
        }
    }
    pub(crate) fn send_rpc(&mut self, op: OperationId, req: session::Request, _kind: ResponseKind) {
        self.send_rpc_raw(op, req, _kind);
    }
    pub(crate) fn send_rpc_raw(&mut self, op: OperationId, mut req: session::Request, kind: ResponseKind) {
        self.rpc_seq += 1;
        self.rpc_req += 1;
        req.req_id = self.rpc_req;
        let id = self.effect(
            op,
            Effect::SendData {
                generation: self.generation,
                data: session::ClientEvent {
                    seq: self.rpc_seq,
                    event: Some(session::client_event::Event::Request(req)),
                }
                .encode_to_vec(),
            },
        );
        if self.pending.contains_key(&id) {
            self.rpc_effects.insert(self.rpc_req, (id, kind));
        }
    }
    pub(crate) fn effect(&mut self, op: OperationId, effect: Effect) -> EffectId {
        let id = self.next_effect;
        self.next_effect += 1;
        if op != 0 && !self.operations.contains_key(&op) {
            return id;
        }
        if self.pending.len() >= self.cfg.max_pending || self.outputs.len() >= self.cfg.max_outputs {
            self.complete(op, Err(ClientError::QueueFull));
            return id;
        }
        self.pending.insert(
            id,
            Pending {
                operation: op,
                generation: self.generation,
                deadline: self.now.saturating_add(self.cfg.rpc_timeout_ms),
                await_rpc: matches!(effect, Effect::SendData { .. }),
            },
        );
        self.active_effects.insert(id, effect.clone());
        self.outputs.push_back(Output::Effect(EffectEnvelope {
            id,
            generation: self.generation,
            operation: Some(op),
            effect,
        }));
        id
    }
    pub(crate) fn alloc_operation(&mut self) -> OperationId {
        let x = self.next_operation;
        self.next_operation += 1;
        x
    }
    pub(crate) fn event(&mut self, e: ClientEvent) {
        self.outputs.push_back(Output::Event(e));
    }
    /// Settle once and remove every continuation owned by the operation.
    pub(crate) fn complete(&mut self, id: OperationId, result: Result<OperationResult, ClientError>) {
        if id == 0 {
            if let Err(error) = result {
                self.event(ClientEvent::Error(error));
            }
            return;
        }
        if self.operations.remove(&id).is_none() {
            return;
        }
        let failed_connection = result.is_err() && (self.connect_pending == Some(id) || self.restart_pending == Some(id));
        self.pending.retain(|_, pending| pending.operation != id);
        self.active_effects.retain(|effect, _| self.pending.contains_key(effect));
        self.deferred_sender.retain(|_, (operation, _)| *operation != id);
        self.rpc_followups.remove(&id);
        self.rpc_effects.retain(|_, (effect, _)| self.pending.contains_key(effect));
        self.outputs.retain(|output| !matches!(output, Output::Effect(effect) if effect.operation == Some(id)));
        if self.connect_pending == Some(id) {
            self.connect_pending = None;
            self.connect_preparation.clear();
        }
        if self.restart_pending == Some(id) {
            self.restart_pending = None;
        }
        if self.negotiation_pending == Some(id) {
            self.negotiation_pending = None;
            self.pending_local_sdp = None;
            self.pending_remote_sdp = None;
        }
        if failed_connection {
            self.pending_local_sdp = None;
            self.pending_remote_sdp = None;
            self.ice_ready = false;
            self.ice_candidates.clear();
            self.session = SessionState::Failed;
            self.event(ClientEvent::SessionChanged(self.session));
        }
        self.event(ClientEvent::OperationCompleted { id, result });
        if failed_connection {
            self.queue_close_peer();
        }
    }
    pub(crate) fn cancel_operations(&mut self) {
        let operations: Vec<_> = self.operations.keys().copied().collect();
        for operation in operations {
            self.complete(operation, Err(ClientError::InvalidState("session disconnected")));
        }
        self.pending.clear();
        self.active_effects.clear();
        self.rpc_effects.clear();
        self.deferred_sender.clear();
        self.rpc_followups.clear();
        self.outputs
            .retain(|output| !matches!(output, Output::Effect(effect) if !matches!(effect.effect, Effect::ClosePeer { .. })));
        self.pending_local_sdp = None;
        self.pending_remote_sdp = None;
    }
    pub(crate) fn queue_close_peer(&mut self) {
        if self.generation > 0 {
            self.outputs.push_back(Output::Effect(EffectEnvelope {
                id: self.next_effect,
                generation: self.generation,
                operation: None,
                effect: Effect::ClosePeer { generation: self.generation },
            }));
            self.next_effect += 1;
        }
    }
    pub(crate) fn close_local(&mut self, op: OperationId) {
        self.ice_ready = false;
        self.ice_candidates.clear();
        self.session = SessionState::Closed;
        self.event(ClientEvent::SessionChanged(self.session));
        self.queue_close_peer();
        self.complete(op, Ok(OperationResult::Disconnected));
    }
}
