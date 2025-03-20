use std::{
    fmt::Debug,
    io::IoSliceMut,
    net::{SocketAddr, SocketAddrV4},
    ops::DerefMut,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use atm0s_sdn::services::async_socket::AsyncSocket;
use quinn::{
    udp::{EcnCodepoint, RecvMeta, Transmit},
    AsyncUdpSocket, UdpPoller,
};

#[derive(Debug)]
pub struct Poller {}

impl UdpPoller for Poller {
    fn poll_writable(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<std::io::Result<()>> {
        //TODO implement this for better performance
        Poll::Ready(Ok(()))
    }
}

pub struct VirtualUdpSocket {
    socket: AsyncSocket,
    addr: SocketAddr,
}

impl VirtualUdpSocket {
    pub fn new(node_id: u32, port: u16, socket: AsyncSocket) -> Self {
        Self {
            addr: SocketAddr::V4(SocketAddrV4::new(node_id.into(), port)),
            socket,
        }
    }
}

impl Debug for VirtualUdpSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualUdpSocket").finish()
    }
}

impl AsyncUdpSocket for VirtualUdpSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        Box::into_pin(Box::new(Poller {}))
    }

    fn try_send(&self, transmit: &Transmit) -> std::io::Result<()> {
        match transmit.destination {
            SocketAddr::V4(addr) => {
                log::debug!("{} sending {} bytes to {}", self.addr, transmit.contents.len(), addr);
                let target_node = u32::from_be_bytes(addr.ip().octets());
                self.socket
                    .send_to(transmit.contents.to_vec().into(), target_node, addr.port(), transmit.ecn.map(|x| x as u8).unwrap_or(0));
                Ok(())
            }
            _ => Err(std::io::ErrorKind::ConnectionRefused.into()),
        }
    }

    fn poll_recv(&self, cx: &mut Context, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> Poll<std::io::Result<usize>> {
        let msg = futures::ready!(self.socket.recv_from(cx)).ok_or(std::io::ErrorKind::BrokenPipe.into())?;
        let len = msg.data.len();
        if len <= bufs[0].len() {
            let addr = SocketAddr::V4(SocketAddrV4::new(msg.remote_node.into(), msg.remote_port));
            log::debug!("[VirtualUdpSocket {}] received {} bytes from {}", self.addr, len, addr);
            bufs[0].deref_mut()[0..len].copy_from_slice(&msg.data);
            meta[0] = quinn::udp::RecvMeta {
                addr,
                len,
                stride: len,
                ecn: if msg.meta == 0 {
                    None
                } else {
                    EcnCodepoint::from_bits(msg.meta)
                },
                dst_ip: None,
            };
            std::task::Poll::Ready(Ok(1))
        } else {
            log::warn!("[VirtualUdpSocket {}] buffer too small for packet {} vs {}, dropping", self.addr, len, bufs[0].len());
            std::task::Poll::Pending
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.addr)
    }
}
