use super::*;

impl Client {
    pub fn command(&mut self, now: Timestamp, command: Command) -> Result<OperationId, ClientError> {
        self.now = now;
        // Stop accepting work before mutating state; queued results are never evicted.
        if !matches!(command, Command::Disconnect) && (self.outputs.len() >= self.cfg.max_outputs || self.operations.len() >= self.cfg.max_pending) {
            return Err(ClientError::QueueFull);
        }
        if matches!(command, Command::Disconnect) {
            self.cancel_operations();
        }
        let op = self.alloc_operation();
        self.operations.insert(op, now.saturating_add(self.cfg.rpc_timeout_ms));
        let result = self.command_inner(op, command);
        if result.is_err() {
            self.operations.remove(&op);
        }
        result
    }

    fn command_inner(&mut self, op: OperationId, command: Command) -> Result<OperationId, ClientError> {
        match command {
            Command::CreateSender { name, kind, config } => {
                if self.session == SessionState::Connected && self.negotiation_pending.is_some() {
                    return Err(ClientError::InvalidState("negotiation in progress"));
                }
                if self.senders.iter().any(|x| x.name == name) {
                    return Err(ClientError::InvalidInput("duplicate sender".into()));
                }
                let connected = self.session == SessionState::Connected;
                let simulcast = config.simulcast;
                self.senders.push(SenderState {
                    name: name.clone(),
                    kind,
                    source: None,
                    source_metadata: None,
                    source_screen: false,
                    config: config.clone(),
                });
                if connected {
                    self.negotiation_pending = Some(op);
                    self.effect(
                        op,
                        Effect::AddTransceiver {
                            generation: self.generation,
                            name: name.clone(),
                            kind,
                            direction: Direction::SendOnly,
                            simulcast,
                        },
                    );
                }
                if !connected {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::CreateReceiver { name, kind, config } => {
                if self.session == SessionState::Connected && self.negotiation_pending.is_some() {
                    return Err(ClientError::InvalidState("negotiation in progress"));
                }
                if self.receivers.iter().any(|x| x.name == name) {
                    return Err(ClientError::InvalidInput("duplicate receiver".into()));
                }
                let connected = self.session == SessionState::Connected;
                self.receivers.push(ReceiverState {
                    name: name.clone(),
                    kind,
                    source: None,
                    config,
                    ready: false,
                });
                if connected {
                    self.negotiation_pending = Some(op);
                    self.effect(
                        op,
                        Effect::AddTransceiver {
                            generation: self.generation,
                            name: name.clone(),
                            kind,
                            direction: Direction::RecvOnly,
                            simulcast: false,
                        },
                    );
                }
                if !connected {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::AttachSender { name, source_id } => {
                let s = self.senders.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown sender".into()))?;
                s.source = Some(source_id.clone());
                let snapshot = s.clone();
                if self.session == SessionState::Connected {
                    let replace = self.effect(
                        op,
                        Effect::ReplaceTrack {
                            generation: self.generation,
                            name: name.clone(),
                            source_id: Some(source_id),
                        },
                    );
                    if self.pending.contains_key(&replace) {
                        self.deferred_sender.insert(replace, (op, sender_request(&snapshot, SenderAction::Attach)));
                    }
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::AttachSenderWithMetadata { name, source_id, metadata, screen } => {
                let s = self.senders.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown sender".into()))?;
                s.source = Some(source_id.clone());
                s.source_metadata = metadata;
                s.source_screen = screen;
                let snapshot = s.clone();
                if self.session == SessionState::Connected {
                    let replace = self.effect(
                        op,
                        Effect::ReplaceTrack {
                            generation: self.generation,
                            name: name.clone(),
                            source_id: Some(source_id),
                        },
                    );
                    if self.pending.contains_key(&replace) {
                        self.deferred_sender.insert(replace, (op, sender_request(&snapshot, SenderAction::Attach)));
                    }
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::DetachSender { name } => {
                let s = self.senders.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown sender".into()))?;
                s.source = None;
                s.source_metadata = None;
                s.source_screen = false;
                let snapshot = s.clone();
                if self.session == SessionState::Connected {
                    let replace = self.effect(
                        op,
                        Effect::ReplaceTrack {
                            generation: self.generation,
                            name: name.clone(),
                            source_id: None,
                        },
                    );
                    if self.pending.contains_key(&replace) {
                        self.deferred_sender.insert(replace, (op, sender_request(&snapshot, SenderAction::Detach)));
                    }
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::ConfigureSender { name, config } => {
                let s = self.senders.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown sender".into()))?;
                s.config = config;
                let snapshot = s.clone();
                if self.session == SessionState::Connected {
                    self.send_rpc(op, sender_request(&snapshot, SenderAction::Config), ResponseKind::Sender);
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::AttachReceiver { name, source } => {
                let r = self.receivers.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown receiver".into()))?;
                r.source = Some(source);
                let snapshot = r.clone();
                if self.session == SessionState::Connected {
                    self.send_rpc(op, receiver_request(&snapshot, true), ResponseKind::Receiver);
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::DetachReceiver { name } => {
                let r = self.receivers.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown receiver".into()))?;
                r.source = None;
                let snapshot = r.clone();
                if self.session == SessionState::Connected {
                    self.send_rpc(op, receiver_request(&snapshot, false), ResponseKind::Receiver);
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::ConfigureReceiver { name, config } => {
                let r = self.receivers.iter_mut().find(|x| x.name == name).ok_or_else(|| ClientError::InvalidInput("unknown receiver".into()))?;
                r.config = config;
                let snapshot = r.clone();
                if self.session == SessionState::Connected {
                    self.send_rpc(op, receiver_config_request(&snapshot), ResponseKind::Receiver);
                } else {
                    self.complete(op, Ok(OperationResult::Updated));
                }
            }
            Command::Connect { gateway, token, offer, version, join } => {
                if !matches!(self.session, SessionState::New | SessionState::Failed | SessionState::Closed) {
                    return Err(ClientError::InvalidState("connect"));
                }
                self.conn_id = None;
                self.ice_lite = false;
                self.ice_ready = false;
                self.ice_candidates.clear();
                self.gateway = Some(gateway.clone());
                self.token = Some(token.clone());
                self.join_config = join.clone();
                self.pending_local_sdp = offer.clone();
                self.generation += 1;
                self.session = SessionState::Connecting;
                self.connect_pending = Some(op);
                self.connect_version = version;
                self.connect_remote_ready = false;
                self.connect_channel_ready = false;
                self.event(ClientEvent::SessionChanged(self.session));
                self.connect_preparation.clear();
                if offer.is_none() {
                    self.connect_preparation.push_back(Effect::CreatePeer { generation: self.generation });
                }
                for sender in self.senders.iter().filter(|_| offer.is_none()) {
                    self.connect_preparation.push_back(Effect::AddTransceiver {
                        generation: self.generation,
                        name: sender.name.clone(),
                        kind: sender.kind,
                        direction: Direction::SendOnly,
                        simulcast: sender.config.simulcast,
                    });
                    if offer.is_none() {
                        if let Some(source_id) = &sender.source {
                            self.connect_preparation.push_back(Effect::ReplaceTrack {
                                generation: self.generation,
                                name: sender.name.clone(),
                                source_id: Some(source_id.clone()),
                            });
                        }
                    }
                }
                for receiver in self.receivers.iter().filter(|_| offer.is_none()) {
                    self.connect_preparation.push_back(Effect::AddTransceiver {
                        generation: self.generation,
                        name: receiver.name.clone(),
                        kind: receiver.kind,
                        direction: Direction::RecvOnly,
                        simulcast: false,
                    });
                }
                self.advance_connect(op);
            }
            Command::RestartIce { offer } => {
                if self.conn_id.is_none() {
                    return Err(ClientError::InvalidState("restart ICE"));
                }
                if self.restart_pending.is_some() || self.negotiation_pending.is_some() || !matches!(self.session, SessionState::Connected | SessionState::Reconnecting) {
                    return Err(ClientError::InvalidState("restart ICE"));
                }
                self.ice_ready = false;
                self.ice_candidates.clear();
                self.session = SessionState::Reconnecting;
                self.restart_pending = Some(op);
                self.pending_local_sdp = offer.clone();
                self.event(ClientEvent::SessionChanged(self.session));
                if let Some(offer) = offer {
                    self.send_connection_request(op, &offer);
                } else {
                    self.effect(
                        op,
                        Effect::CreateOffer {
                            generation: self.generation,
                            restart: true,
                        },
                    );
                }
            }
            Command::Join { info, token } => {
                if self.session != SessionState::Connected {
                    return Err(ClientError::InvalidState("join"));
                }
                self.token = Some(token.clone());
                self.join_config = Some(info.clone());
                self.room = Some(RoomInfo {
                    room: info.room.clone(),
                    peer: info.peer.clone(),
                    metadata: info.metadata.clone(),
                });
                self.event(ClientEvent::RoomChanged(self.room.clone()));
                let req = session::Request {
                    req_id: 0,
                    request: Some(session::request::Request::Session(session::request::Session {
                        request: Some(session::request::session::Request::Join(session::request::session::Join {
                            info: Some(make_join_proto(&info)),
                            token,
                        })),
                    })),
                };
                self.send_rpc_raw(op, req, ResponseKind::Session);
            }
            Command::Leave => {
                let req = session_request(session::request::session::Request::Leave(session::request::session::Leave {}));
                self.send_rpc(op, req, ResponseKind::Session);
                self.room = None;
                self.join_config = None;
                self.event(ClientEvent::RoomChanged(None));
            }
            Command::CreateMessageChannel { label, publish } => {
                self.channels.insert(label.clone(), publish);
                if publish {
                    self.rpc_followups.insert(op, message_request(&label, Some(MessageAction::StartPub), None));
                    self.send_rpc(op, message_request(&label, Some(MessageAction::Sub), None), ResponseKind::Message);
                } else {
                    self.send_rpc(op, message_request(&label, Some(MessageAction::Sub), None), ResponseKind::Message);
                }
            }
            Command::SetMessagePublishing { label, enabled } => {
                if !self.channels.contains_key(&label) {
                    return Err(ClientError::InvalidInput("unknown channel".into()));
                }
                self.channels.insert(label.clone(), enabled);
                let req = message_request(
                    &label,
                    Some(if enabled {
                        MessageAction::StartPub
                    } else {
                        MessageAction::StopPub
                    }),
                    None,
                );
                self.send_rpc(op, req, ResponseKind::Message);
            }
            Command::PublishMessage { label, data } => {
                if data.len() > self.cfg.max_message_size {
                    return Err(ClientError::InvalidInput("message too large".into()));
                }
                if self.channels.get(&label) != Some(&true) {
                    return Err(ClientError::InvalidState("channel is not publishing"));
                }
                self.send_rpc(op, message_request(&label, Some(MessageAction::Pub), Some(data)), ResponseKind::Message);
            }
            Command::CloseMessageChannel { label } => {
                let was_publishing = self.channels.remove(&label).unwrap_or(false);
                if was_publishing {
                    self.rpc_followups.insert(op, message_request(&label, Some(MessageAction::Unsub), None));
                    self.send_rpc(op, message_request(&label, Some(MessageAction::StopPub), None), ResponseKind::Message);
                } else {
                    self.send_rpc(op, message_request(&label, Some(MessageAction::Unsub), None), ResponseKind::Message);
                }
            }
            Command::MixerAttach { sources } => {
                self.send_rpc(op, feature_request(sources, true), ResponseKind::Mixer);
            }
            Command::MixerDetach { sources } => {
                self.send_rpc(op, feature_request(sources, false), ResponseKind::Mixer);
            }
            Command::Disconnect => {
                if self.session == SessionState::Closed {
                    self.complete(op, Ok(OperationResult::Disconnected));
                } else {
                    self.close_local(op);
                }
            }
        }
        Ok(op)
    }
}
