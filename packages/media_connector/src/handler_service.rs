use std::{collections::VecDeque, fmt::Debug};

use atm0s_sdn::{
    base::{
        NetOutgoingMeta, Service, ServiceBuilder, ServiceControlActor, ServiceCtx, ServiceInput, ServiceOutput, ServiceSharedInput, ServiceWorker, ServiceWorkerCtx, ServiceWorkerInput,
        ServiceWorkerOutput,
    },
    features::{data, FeaturesControl, FeaturesEvent},
    NodeId, RouteRule,
};
use media_server_protocol::protobuf::cluster_connector::{connector_request, connector_response, ConnectorRequest, ConnectorResponse};
use prost::Message;

use crate::{DATA_PORT, HANDLER_SERVICE_ID, HANDLER_SERVICE_NAME};

#[derive(Debug, Clone)]
pub enum Control {
    Sub,
    Res(NodeId, u64, connector_response::Response),
}

#[derive(Debug, Clone)]
pub enum Event {
    Req(NodeId, u64, u64, connector_request::Request),
}

pub struct ConnectorHandlerService<UserData, SC, SE, TC, TW> {
    subscriber: Option<ServiceControlActor<UserData>>,
    queue: VecDeque<ServiceOutput<UserData, FeaturesControl, SE, TW>>,
    shutdown: bool,
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> Default for ConnectorHandlerService<UserData, SC, SE, TC, TW> {
    fn default() -> Self {
        Self::new()
    }
}

impl<UserData, SC, SE, TC, TW> ConnectorHandlerService<UserData, SC, SE, TC, TW> {
    pub fn new() -> Self {
        Self {
            subscriber: None,
            queue: VecDeque::from([ServiceOutput::FeatureControl(data::Control::DataListen(DATA_PORT).into())]),
            shutdown: false,
            _tmp: std::marker::PhantomData,
        }
    }
}

impl<UserData: Copy + Eq, SC, SE, TC, TW> Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorHandlerService<UserData, SC, SE, TC, TW>
where
    SC: From<Control> + TryInto<Control> + Debug,
    SE: From<Event> + TryInto<Event>,
{
    fn service_id(&self) -> u8 {
        HANDLER_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        HANDLER_SERVICE_NAME
    }

    fn is_service_empty(&self) -> bool {
        self.shutdown && self.queue.is_empty()
    }

    fn on_shared_input<'a>(&mut self, _ctx: &ServiceCtx, _now: u64, _input: ServiceSharedInput) {}

    fn on_input(&mut self, _ctx: &ServiceCtx, _now: u64, input: ServiceInput<UserData, FeaturesEvent, SC, TC>) {
        match input {
            ServiceInput::Control(owner, control) => {
                if let Ok(control) = control.try_into() {
                    match control {
                        Control::Sub => {
                            self.subscriber = Some(owner);
                        }
                        Control::Res(source, req_id, res) => {
                            let res = ConnectorResponse { req_id, response: Some(res) };
                            log::info!("[ConnectorHandler] reply to net {:?}", res);
                            self.queue.push_back(ServiceOutput::FeatureControl(
                                data::Control::DataSendRule(DATA_PORT, RouteRule::ToNode(source), NetOutgoingMeta::secure(), res.encode_to_vec()).into(),
                            ));
                        }
                    }
                }
            }
            ServiceInput::FromWorker(_data) => {}
            ServiceInput::FeatureEvent(FeaturesEvent::Data(event)) => match event {
                data::Event::Pong(_, _) => {}
                data::Event::Recv(_port, meta, buf) => match ConnectorRequest::decode(buf.as_slice()) {
                    Ok(msg) => {
                        if let Some(source) = meta.source {
                            log::info!("[ConnectorHandler] on event {:?}", msg);
                            if let Some(event) = msg.request {
                                if let Some(actor) = self.subscriber {
                                    self.queue.push_back(ServiceOutput::Event(actor, Event::Req(source, msg.ts, msg.req_id, event).into()));
                                } else {
                                    log::warn!("[ConnectorHandler] subscriber not found");
                                }
                            }
                        } else {
                            log::warn!("[ConnectorHandler] reject msg without source");
                        }
                    }
                    Err(er) => {
                        log::error!("[ConnectorHandler] decode data error {}", er);
                    }
                },
            },
            _ => {}
        }
    }

    fn on_shutdown(&mut self, _ctx: &ServiceCtx, _now: u64) {
        self.shutdown = true;
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceOutput<UserData, FeaturesControl, SE, TW>> {
        self.queue.pop_front()
    }
}

pub struct ConnectorHandlerServiceWorker<UserData, SC, SE, TC> {
    queue: VecDeque<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>>,
    shutdown: bool,
}

impl<UserData, SC, SE, TC, TW> ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorHandlerServiceWorker<UserData, SC, SE, TC> {
    fn service_id(&self) -> u8 {
        HANDLER_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        HANDLER_SERVICE_NAME
    }

    fn is_service_empty(&self) -> bool {
        self.shutdown && self.queue.is_empty()
    }

    fn on_tick(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, _tick_count: u64) {}

    fn on_input(&mut self, _ctx: &ServiceWorkerCtx, _now: u64, input: ServiceWorkerInput<UserData, FeaturesEvent, SC, TW>) {
        match input {
            ServiceWorkerInput::Control(owner, control) => self.queue.push_back(ServiceWorkerOutput::ForwardControlToController(owner, control)),
            ServiceWorkerInput::FromController(_) => {}
            ServiceWorkerInput::FeatureEvent(event) => self.queue.push_back(ServiceWorkerOutput::ForwardFeatureEventToController(event)),
        }
    }

    fn on_shutdown(&mut self, _ctx: &ServiceWorkerCtx, _now: u64) {
        self.shutdown = true;
    }

    fn pop_output2(&mut self, _now: u64) -> Option<ServiceWorkerOutput<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC>> {
        self.queue.pop_front()
    }
}

pub struct ConnectorHandlerServiceBuilder<UserData, SC, SE, TC, TW> {
    _tmp: std::marker::PhantomData<(UserData, SC, SE, TC, TW)>,
}

impl<UserData, SC, SE, TC, TW> Default for ConnectorHandlerServiceBuilder<UserData, SC, SE, TC, TW> {
    fn default() -> Self {
        Self::new()
    }
}

impl<UserData, SC, SE, TC, TW> ConnectorHandlerServiceBuilder<UserData, SC, SE, TC, TW> {
    pub fn new() -> Self {
        Self { _tmp: std::marker::PhantomData }
    }
}

impl<UserData, SC, SE, TC, TW> ServiceBuilder<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW> for ConnectorHandlerServiceBuilder<UserData, SC, SE, TC, TW>
where
    UserData: 'static + Debug + Send + Sync + Copy + Eq,
    SC: 'static + Debug + Send + Sync + From<Control> + TryInto<Control>,
    SE: 'static + Debug + Send + Sync + From<Event> + TryInto<Event>,
    TC: 'static + Debug + Send + Sync,
    TW: 'static + Debug + Send + Sync,
{
    fn service_id(&self) -> u8 {
        HANDLER_SERVICE_ID
    }

    fn service_name(&self) -> &str {
        HANDLER_SERVICE_NAME
    }

    fn discoverable(&self) -> bool {
        true
    }

    fn create(&self) -> Box<dyn Service<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(ConnectorHandlerService::new())
    }

    fn create_worker(&self) -> Box<dyn ServiceWorker<UserData, FeaturesControl, FeaturesEvent, SC, SE, TC, TW>> {
        Box::new(ConnectorHandlerServiceWorker {
            queue: Default::default(),
            shutdown: false,
        })
    }
}
