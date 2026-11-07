use super::*;

impl Client {
    pub fn on_input(&mut self, now: Timestamp, input: Input) {
        self.now = now;
        match input {
            Input::Tick => {
                let expired: Vec<_> = self.operations.iter().filter(|(_, deadline)| **deadline <= now).map(|(operation, _)| *operation).collect();
                for operation in expired {
                    self.complete(operation, Err(ClientError::Timeout { operation }));
                }
                let expired_background: Vec<_> = self
                    .pending
                    .iter()
                    .filter(|(_, pending)| pending.operation == 0 && pending.deadline <= now)
                    .map(|(effect, _)| *effect)
                    .collect();
                for effect in expired_background {
                    self.pending.remove(&effect);
                    self.active_effects.remove(&effect);
                    self.rpc_effects.retain(|_, (id, _)| *id != effect);
                    self.complete(0, Err(ClientError::Timeout { operation: 0 }));
                }
            }
            Input::Rtc(e) => {
                if e.generation != self.generation || matches!(self.session, SessionState::Closed | SessionState::Failed) {
                    return;
                }
                match e.event {
                    RtcEvent::Data(bytes) => self.on_data(bytes),
                    RtcEvent::DataChannelOpen => {
                        self.connect_channel_ready = true;
                        self.finish_connect();
                    }
                    RtcEvent::LocalCandidate(candidate) => self.local_candidate(candidate),
                    RtcEvent::TrackReady { name, track_id } => {
                        if let Some(r) = self.receivers.iter_mut().find(|r| r.name == name) {
                            r.ready = true;
                        }
                        self.event(ClientEvent::ReceiverReady { name, track_id });
                    }
                    RtcEvent::ConnectionState(s) if s == "disconnected" => {
                        self.session = SessionState::Reconnecting;
                        self.event(ClientEvent::SessionChanged(self.session));
                    }
                    RtcEvent::NegotiationNeeded => {
                        if self.session == SessionState::Connected && self.negotiation_pending.is_none() {
                            let operation = self.alloc_operation();
                            self.operations.insert(operation, self.now.saturating_add(self.cfg.rpc_timeout_ms));
                            self.negotiation_pending = Some(operation);
                            self.effect(
                                operation,
                                Effect::CreateOffer {
                                    generation: self.generation,
                                    restart: false,
                                },
                            );
                        }
                    }
                    RtcEvent::ConnectionState(_) => {}
                }
            }
            Input::EffectCompleted(c) => {
                let Some(p) = self.pending.get(&c.id) else { return };
                if c.generation != self.generation || c.generation != p.generation {
                    return;
                }
                if p.await_rpc {
                    if let Err(e) = c.result {
                        let p = self.pending.remove(&c.id).unwrap();
                        self.active_effects.remove(&c.id);
                        self.rpc_effects.retain(|_, (id, _)| *id != c.id);
                        self.complete(p.operation, Err(e));
                    }
                    return;
                }
                let p = self.pending.remove(&c.id).unwrap();
                let effect = self.active_effects.remove(&c.id);
                if let Some((deferred_op, request)) = self.deferred_sender.remove(&c.id) {
                    match c.result {
                        Ok(_) => self.send_rpc_raw(deferred_op, request, ResponseKind::Sender),
                        Err(error) => self.complete(deferred_op, Err(error)),
                    }
                    return;
                }
                match c.result {
                    Ok(result) => {
                        if self.connect_pending == Some(p.operation) && matches!(effect, Some(Effect::CreatePeer { .. } | Effect::AddTransceiver { .. } | Effect::ReplaceTrack { .. })) {
                            self.advance_connect(p.operation);
                        } else if matches!(effect, Some(Effect::AddTransceiver { .. })) {
                            if self.negotiation_pending == Some(p.operation) {
                                self.effect(
                                    p.operation,
                                    Effect::CreateOffer {
                                        generation: self.generation,
                                        restart: false,
                                    },
                                );
                            }
                        } else if matches!(effect, Some(Effect::Http(_) | Effect::CreateOffer { .. })) {
                            self.effect_ok(p.operation, result)
                        } else if matches!(effect, Some(Effect::SetLocalDescription { .. })) {
                            if let Some(sdp) = self.pending_remote_sdp.take() {
                                self.effect(p.operation, Effect::SetRemoteDescription { generation: self.generation, sdp });
                            } else if self.negotiation_pending == Some(p.operation) {
                                let offer = self.pending_local_sdp.clone().unwrap_or_default();
                                self.send_rpc(p.operation, update_sdp_request(&self.senders, &self.receivers, &offer), ResponseKind::Session);
                            }
                        } else if matches!(effect, Some(Effect::SetRemoteDescription { .. })) {
                            if self.connect_pending == Some(p.operation) {
                                self.connect_remote_ready = true;
                                self.finish_connect();
                            } else if self.restart_pending == Some(p.operation) {
                                self.restart_pending = None;
                                self.session = SessionState::Connected;
                                self.event(ClientEvent::SessionChanged(self.session));
                                self.complete(p.operation, Ok(OperationResult::Connected));
                            } else if self.negotiation_pending == Some(p.operation) {
                                self.negotiation_pending = None;
                                self.pending_remote_sdp = None;
                                self.complete(p.operation, Ok(OperationResult::Updated));
                            }
                        } else if p.operation != 0 {
                            self.complete(p.operation, Ok(OperationResult::Updated))
                        }
                    }
                    Err(e) => self.complete(p.operation, Err(e)),
                }
            }
        }
    }
}
