//! Nonblocking live UDP. DNS and socket preparation happen in `PreparedUpdate`
//! on the control thread; `apply` only swaps prepared state. Replay never calls
//! this module and therefore cannot forward replay input.
use super::{DatagramTap, Disposition, Ingest, IngestStatus, config::ForwardConfig};
use crate::telemetry::{ParseError, ParsedDatagram};
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
};

/// A poll yields after this many receives even under continuous input.
pub const MAX_DATAGRAMS_PER_POLL: usize = 256;

#[derive(Debug)]
pub struct ReceivedDatagram {
    pub arrival_time_us: u64,
    pub result: Result<(ParsedDatagram, Disposition), ParseError>,
}

#[derive(Debug, Clone)]
pub struct LiveStatus {
    pub ingest: IngestStatus,
    pub forwarded: u64,
    pub forwarding_errors: u64,
}

#[derive(Default)]
struct Forwarding {
    targets: Vec<SocketAddr>,
    ipv4: Option<UdpSocket>,
    ipv6: Option<UdpSocket>,
}

impl Forwarding {
    fn prepare(config: &ForwardConfig) -> io::Result<Self> {
        let mut forwarding = Self::default();
        if !config.enabled {
            return Ok(forwarding);
        }
        for target in &config.targets {
            // One resolved address per configured destination; sending to all
            // DNS answers would duplicate packets at a dual-stack receiver.
            let address = target.to_socket_addrs()?.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("no address for {target}"),
                )
            })?;
            if address.port() == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "forwarding port must be nonzero",
                ));
            }
            if !forwarding.targets.contains(&address) {
                forwarding.targets.push(address);
            }
        }
        if forwarding.targets.iter().any(SocketAddr::is_ipv4) {
            forwarding.ipv4 = Some(bind("0.0.0.0:0".parse().unwrap())?);
        }
        if forwarding.targets.iter().any(SocketAddr::is_ipv6) {
            forwarding.ipv6 = Some(bind("[::]:0".parse().unwrap())?);
        }
        Ok(forwarding)
    }

    fn send(&self, bytes: &[u8]) -> (u64, u64) {
        let mut sent = 0;
        let mut errors = 0;
        for target in &self.targets {
            let socket = if target.is_ipv4() {
                &self.ipv4
            } else {
                &self.ipv6
            };
            match socket
                .as_ref()
                .expect("prepared forwarding socket")
                .send_to(bytes, target)
            {
                Ok(length) if length == bytes.len() => sent += 1,
                _ => errors += 1, // Includes WouldBlock; never retry or hold up receive.
            }
        }
        (sent, errors)
    }
}

fn bind(address: SocketAddr) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(address)
        .map_err(|error| io::Error::new(error.kind(), format!("UDP bind {address}: {error}")))?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

/// Prepare away from the receive loop. Failure leaves the old listener and
/// forwarding active. Forward-only changes keep the listener and queued input.
pub struct PreparedUpdate {
    socket: Option<UdpSocket>,
    forwarding: Forwarding,
}
impl PreparedUpdate {
    pub fn prepare(
        current: SocketAddr,
        desired: SocketAddr,
        config: &ForwardConfig,
    ) -> io::Result<Self> {
        let forwarding = Forwarding::prepare(config)?;
        let socket = if current != desired {
            Some(bind(desired)?)
        } else {
            None
        };
        Ok(Self { socket, forwarding })
    }
}

pub struct LiveUdp {
    socket: UdpSocket,
    forwarding: Forwarding,
    ingest: Ingest,
    forwarded: u64,
    forwarding_errors: u64,
}

impl LiveUdp {
    pub fn bind(address: SocketAddr, config: &ForwardConfig) -> io::Result<Self> {
        let forwarding = Forwarding::prepare(config)?;
        Ok(Self {
            socket: bind(address)?,
            forwarding,
            ingest: Ingest::default(),
            forwarded: 0,
            forwarding_errors: 0,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn apply(&mut self, update: PreparedUpdate) {
        if let Some(socket) = update.socket {
            self.socket = socket;
        }
        self.forwarding = update.forwarding;
    }

    pub fn status(&self, now_us: u64) -> LiveStatus {
        LiveStatus {
            ingest: self.ingest.status(now_us),
            forwarded: self.forwarded,
            forwarding_errors: self.forwarding_errors,
        }
    }

    /// Read at most `budget` datagrams (capped at MAX_DATAGRAMS_PER_POLL),
    /// stopping immediately on WouldBlock. The arrival clock and raw tap must
    /// not wait or perform I/O. The returned Apply inputs are the #31 handoff.
    pub fn poll(
        &mut self,
        budget: usize,
        mut now_us: impl FnMut() -> u64,
        tap: &mut (impl DatagramTap + ?Sized),
    ) -> io::Result<Vec<ReceivedDatagram>> {
        // Large enough for any UDP datagram; never truncate to a known packet size.
        let mut bytes = [0u8; 65535];
        let mut received = Vec::new();
        for _ in 0..budget.min(MAX_DATAGRAMS_PER_POLL) {
            let length = match self.socket.recv_from(&mut bytes) {
                Ok((length, _)) => length,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            let arrival_time_us = now_us();
            let bytes = &bytes[..length];
            let (sent, errors) = self.forwarding.send(bytes);
            self.forwarded += sent;
            self.forwarding_errors += errors;
            let result = self.ingest.ingest(arrival_time_us, bytes, tap);
            received.push(ReceivedDatagram {
                arrival_time_us,
                result,
            });
        }
        Ok(received)
    }
}
