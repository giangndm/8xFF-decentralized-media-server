use crate::{Client, ClientError, ClientEvent, DriverError, EffectCompletion, Input, IoDriver, Output};
use std::collections::VecDeque;

/// Generic synchronous pump. Browser adapters may use the same core types
/// directly when effects are resolved by JavaScript promises.
pub struct ClientRuntime<D: IoDriver> {
    pub client: Client,
    pub driver: D,
    events: VecDeque<ClientEvent>,
}

impl<D: IoDriver> ClientRuntime<D> {
    pub fn new(client: Client, driver: D) -> Self {
        Self {
            client,
            driver,
            events: VecDeque::new(),
        }
    }
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }
    pub fn pump(&mut self) -> Result<(), DriverError> {
        for _ in 0..1024 {
            let mut progressed = false;
            while let Some(input) = self.driver.poll_input() {
                progressed = true;
                self.client.on_input(self.driver.now(), input);
            }
            while let Some(output) = self.client.poll_output() {
                progressed = true;
                match output {
                    Output::Event(event) => self.events.push_back(event),
                    Output::Effect(effect) => {
                        if let Err(error) = self.driver.submit(effect.clone()) {
                            self.client.on_input(
                                self.driver.now(),
                                Input::EffectCompleted(EffectCompletion {
                                    id: effect.id,
                                    generation: effect.generation,
                                    result: Err(ClientError::Transport(error.0)),
                                }),
                            );
                        }
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        self.driver.schedule_wakeup(self.client.next_deadline());
        Ok(())
    }
}
