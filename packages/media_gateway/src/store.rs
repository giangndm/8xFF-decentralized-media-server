use atm0s_sdn::NodeId;
use media_server_protocol::{
    cluster::ZoneId,
    protobuf::cluster_gateway::ping_event::{gateway_origin::Location, GatewayOrigin, Origin, ServiceStats},
};

use crate::{NodeMetrics, ServiceKind};

use self::service::ServiceStore;

mod service;

#[derive(Debug, PartialEq)]
pub struct PingEvent {
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub origin: Origin,
    pub webrtc: Option<ServiceStats>,
    pub rtpengine: Option<ServiceStats>,
}

pub struct GatewayStore {
    zone: ZoneId,
    node: NodeMetrics,
    location: Location,
    webrtc: ServiceStore,
    rtpengine: ServiceStore,
    output: Option<PingEvent>,
    max_cpu: u8,
    max_memory: u8,
    max_disk: u8,
}

impl GatewayStore {
    pub fn new(zone: ZoneId, location: Location, max_cpu: u8, max_memory: u8, max_disk: u8) -> Self {
        Self {
            node: NodeMetrics::default(),
            webrtc: ServiceStore::new(zone, ServiceKind::Webrtc, location),
            rtpengine: ServiceStore::new(zone, ServiceKind::RtpEngine, location),
            zone,
            location,
            output: None,
            max_cpu,
            max_disk,
            max_memory,
        }
    }

    pub fn on_node_metrics(&mut self, _now: u64, metrics: NodeMetrics) {
        self.node = metrics;
    }

    pub fn on_tick(&mut self, now: u64) {
        self.webrtc.on_tick(now);
        self.rtpengine.on_tick(now);

        let ping = PingEvent {
            cpu: self.node.cpu,
            memory: self.node.memory,
            disk: self.node.disk,
            origin: Origin::Gateway(GatewayOrigin {
                zone: self.zone.0,
                location: Some(self.location),
            }),
            webrtc: self.webrtc.local_stats(),
            rtpengine: self.rtpengine.local_stats(),
        };

        log::trace!("[GatewayStore] create ping event for broadcast {:?}", ping);
        self.output = Some(ping);
    }

    pub fn on_ping(&mut self, now: u64, from: u32, ping: PingEvent) {
        log::debug!("[GatewayStore] on ping from {from} data {:?}", ping);
        let node_usage = node_usage(&ping, self.max_cpu, self.max_memory, self.max_disk);
        let webrtc_usage = webrtc_usage(&ping, self.max_cpu, self.max_memory, self.max_disk);
        let rtpengine_usage = rtpengine_usage(&ping, self.max_cpu, self.max_memory, self.max_disk);
        match ping.origin {
            Origin::Media(_) => {
                match (node_usage, webrtc_usage, ping.webrtc) {
                    (Some(_node), Some(webrtc), Some(stats)) => self.webrtc.on_node_ping(now, from, webrtc, stats),
                    e => {
                        log::warn!("[GatewayStore] remove node from webrtc because usage too high {:?}", e);
                        self.webrtc.remove_node(from);
                    }
                }
                match (node_usage, rtpengine_usage, ping.rtpengine) {
                    (Some(_node), Some(rtpengine), Some(stats)) => self.rtpengine.on_node_ping(now, from, rtpengine, stats),
                    e => {
                        log::warn!("[GatewayStore] remove node from rtpengine because usage too high {:?}", e);
                        self.rtpengine.remove_node(from);
                    }
                }
            }
            Origin::Gateway(gateway) => {
                if gateway.zone == self.zone.0 {
                    //Reject stats from same zone
                    return;
                }
                match (node_usage, webrtc_usage, gateway.location, ping.webrtc) {
                    (Some(node), Some(webrtc), Some(location), Some(stats)) => self.webrtc.on_gateway_ping(now, ZoneId(gateway.zone), from, node, location, webrtc, stats),
                    _ => {
                        self.webrtc.remove_gateway(ZoneId(gateway.zone), from);
                        self.rtpengine.remove_gateway(ZoneId(gateway.zone), from);
                    }
                }
            }
        }
    }

    pub fn best_for(&self, kind: ServiceKind, location: Option<Location>) -> Option<NodeId> {
        let node = match kind {
            ServiceKind::Webrtc => self.webrtc.best_for(location),
            ServiceKind::RtpEngine => self.rtpengine.best_for(location),
        };
        log::debug!("[GatewayStore] query best {:?} for {:?} got {:?}", kind, location, node);
        node
    }

    pub fn dest_for(&self, kind: ServiceKind, dest: NodeId) -> Option<NodeId> {
        let node = match kind {
            ServiceKind::Webrtc => self.webrtc.dest_for(dest),
            ServiceKind::RtpEngine => self.rtpengine.dest_for(dest),
        };
        log::debug!("[GatewayStore] query dest {:?} for node {} got {:?}", kind, dest, node);
        node
    }

    pub fn local_stats(&self) -> Option<ServiceStats> {
        self.webrtc.local_stats()
    }

    pub fn pop_output(&mut self) -> Option<PingEvent> {
        self.output.take()
    }
}

fn node_usage(ping: &PingEvent, max_cpu: u8, max_memory: u8, max_disk: u8) -> Option<u8> {
    if ping.cpu >= max_cpu {
        return None;
    }

    if ping.memory >= max_memory {
        return None;
    }

    if ping.disk >= max_disk {
        return None;
    }

    Some(ping.cpu)
}

fn webrtc_usage(ping: &PingEvent, max_cpu: u8, max_memory: u8, max_disk: u8) -> Option<u8> {
    if ping.cpu >= max_cpu {
        return None;
    }

    if ping.memory >= max_memory {
        return None;
    }

    if ping.disk >= max_disk {
        return None;
    }

    let webrtc = ping.webrtc.as_ref()?;
    webrtc.active.then(|| ping.cpu.max(((webrtc.live * 100) / webrtc.max) as u8))
}

fn rtpengine_usage(ping: &PingEvent, max_cpu: u8, max_memory: u8, max_disk: u8) -> Option<u8> {
    if ping.cpu >= max_cpu {
        return None;
    }

    if ping.memory >= max_memory {
        return None;
    }

    if ping.disk >= max_disk {
        return None;
    }

    let rtpengine = ping.rtpengine.as_ref()?;
    rtpengine.active.then(|| ping.cpu.max(((rtpengine.live * 100) / rtpengine.max) as u8))
}

#[cfg(test)]
mod tests {
    use media_server_protocol::{
        cluster::ZoneId,
        protobuf::cluster_gateway::ping_event::{gateway_origin::Location, GatewayOrigin, MediaOrigin, Origin, ServiceStats},
    };

    use crate::ServiceKind;

    use super::{GatewayStore, PingEvent};

    #[test]
    fn local_ping() {
        let mut store = GatewayStore::new(ZoneId(0), Location { lat: 1.0, lon: 1.0 }, 60, 80, 90);
        store.on_ping(
            0,
            1,
            PingEvent {
                cpu: 0,
                memory: 0,
                disk: 0,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        assert_eq!(store.best_for(ServiceKind::Webrtc, None), Some(1));

        assert_eq!(store.pop_output(), None);
        store.on_tick(100);
        assert_eq!(
            store.pop_output(),
            Some(PingEvent {
                cpu: 0,
                memory: 0,
                disk: 0,
                origin: Origin::Gateway(GatewayOrigin {
                    location: Some(Location { lat: 1.0, lon: 1.0 }),
                    zone: 0,
                }),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            })
        );
    }

    #[test]
    fn local_reject_max_usage() {
        let mut store = GatewayStore::new(ZoneId(0), Location { lat: 1.0, lon: 1.0 }, 60, 80, 90);
        store.on_ping(
            0,
            1,
            PingEvent {
                cpu: 10,
                memory: 80,
                disk: 20,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        store.on_ping(
            0,
            2,
            PingEvent {
                cpu: 10,
                memory: 20,
                disk: 90,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        store.on_ping(
            0,
            3,
            PingEvent {
                cpu: 60,
                memory: 80,
                disk: 20,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        assert_eq!(store.best_for(ServiceKind::Webrtc, None), None);
    }

    #[test]
    fn remote_ping() {
        let mut store = GatewayStore::new(ZoneId(0), Location { lat: 1.0, lon: 1.0 }, 60, 80, 90);
        store.on_ping(
            0,
            257,
            PingEvent {
                cpu: 0,
                memory: 0,
                disk: 0,
                origin: Origin::Gateway(GatewayOrigin {
                    location: Some(Location { lat: 2.0, lon: 2.0 }),
                    zone: 256,
                }),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        assert_eq!(store.best_for(ServiceKind::Webrtc, None), Some(257));

        assert_eq!(store.pop_output(), None);
        store.on_tick(100);
        assert_eq!(
            store.pop_output(),
            Some(PingEvent {
                cpu: 0,
                memory: 0,
                disk: 0,
                origin: Origin::Gateway(GatewayOrigin {
                    location: Some(Location { lat: 1.0, lon: 1.0 }),
                    zone: 0,
                }),
                webrtc: None,
                rtpengine: None,
            })
        );
    }

    #[test]
    fn clear_timeout() {
        let mut store = GatewayStore::new(ZoneId(0), Location { lat: 1.0, lon: 1.0 }, 60, 80, 90);
        store.on_ping(
            0,
            1,
            PingEvent {
                cpu: 10,
                memory: 20,
                disk: 30,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        store.on_ping(
            0,
            257,
            PingEvent {
                cpu: 10,
                memory: 20,
                disk: 30,
                origin: Origin::Gateway(GatewayOrigin {
                    location: Some(Location { lat: 2.0, lon: 2.0 }),
                    zone: 1,
                }),
                webrtc: Some(ServiceStats { live: 100, max: 1000, active: true }),
                rtpengine: None,
            },
        );

        // Verify nodes are registered
        assert_eq!(store.best_for(ServiceKind::Webrtc, None), Some(1));

        // Trigger timeout
        store.on_tick(5000); // PING_TIMEOUT is 5000

        // Verify nodes are cleared
        assert_eq!(store.best_for(ServiceKind::Webrtc, None), None);
    }

    #[test]
    fn clear_timeout_rtpengine() {
        let mut store = GatewayStore::new(ZoneId(0), Location { lat: 1.0, lon: 1.0 }, 60, 80, 90);
        store.on_ping(
            0,
            1,
            PingEvent {
                cpu: 10,
                memory: 20,
                disk: 30,
                origin: Origin::Media(MediaOrigin {}),
                webrtc: None,
                rtpengine: Some(ServiceStats { live: 100, max: 1000, active: true }),
            },
        );

        store.on_ping(
            0,
            257,
            PingEvent {
                cpu: 10,
                memory: 20,
                disk: 30,
                origin: Origin::Gateway(GatewayOrigin {
                    location: Some(Location { lat: 2.0, lon: 2.0 }),
                    zone: 1,
                }),
                webrtc: None,
                rtpengine: Some(ServiceStats { live: 100, max: 1000, active: true }),
            },
        );

        // Verify nodes are registered
        assert_eq!(store.best_for(ServiceKind::RtpEngine, None), Some(1));

        // Trigger timeout
        store.on_tick(5000); // PING_TIMEOUT is 5000

        // Verify nodes are cleared
        assert_eq!(store.best_for(ServiceKind::RtpEngine, None), None);
    }
}
