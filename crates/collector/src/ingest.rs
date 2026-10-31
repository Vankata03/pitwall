//! Live datagram processing. Timestamps are monotonic microseconds supplied by
//! the caller; this gate reads no clock and is also usable by replay.
pub mod config;
pub mod runtime;
pub mod udp;
use crate::telemetry::{EventDetails, Packet, ParsedDatagram};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    UidZero,
    Closed,
    Apply,
    DropFrame,
    IgnoreEnded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionIdentity {
    pub session_uid: u64,
    pub packet_format: u16,
}

/// Raw arrival tap for #30. Called once for EVERY input, before parsing or
/// filtering, including duplicates. The recorder owns its own identity/format
/// eligibility and file lifecycle. Enqueue here; never write files or wait.
/// Arrival time is monotonic microseconds from the caller's clock origin.
pub trait DatagramTap {
    fn datagram(&mut self, arrival_time: u64, bytes: &[u8]);
}

impl<F: FnMut(u64, &[u8])> DatagramTap for F {
    fn datagram(&mut self, arrival_time: u64, bytes: &[u8]) {
        self(arrival_time, bytes);
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IngestStatus {
    pub received: u64,
    pub dropped_frames: u64,
    pub uid_zero: u64,
    pub closed_uid: u64,
    pub ignored_ended: u64,
    pub parse_errors: u64,
    pub last_format: Option<u16>,
    pub last_received_us: Option<u64>,
    /// Counts in the preceding one-second window, including frame duplicates.
    pub packet_rates_hz: [f64; 17],
    /// Maximum rate of the menu-rate packet types, not their sum.
    pub packet_rate_hz: f64,
}

#[derive(Default)]
pub struct Ingest {
    active: Option<SessionIdentity>,
    last_frame: HashMap<u8, u32>,
    ended: bool,
    closed: HashSet<SessionIdentity>,
    status: IngestStatus,
    arrivals: VecDeque<(u64, u8)>,
}

impl Ingest {
    /// The raw tap runs first. Only `Disposition::Apply` reaches #31 via
    /// `model.apply(arrival_time, parsed)`. Drive `model.advance(time)` during
    /// silence too. Keep a separate model instance for live and replay.
    pub fn ingest<T: DatagramTap + ?Sized>(
        &mut self,
        arrival_time: u64,
        bytes: &[u8],
        tap: &mut T,
    ) -> Result<(ParsedDatagram, Disposition), crate::telemetry::ParseError> {
        tap.datagram(arrival_time, bytes);
        self.status.received += 1;
        self.status.last_received_us = Some(arrival_time);
        if let Some(format) = bytes.get(..2) {
            self.status.last_format = Some(u16::from_le_bytes([format[0], format[1]]));
        }
        while self
            .arrivals
            .front()
            .is_some_and(|(at, _)| arrival_time.saturating_sub(*at) >= 1_000_000)
        {
            self.arrivals.pop_front();
        }
        let parsed = crate::telemetry::parse_datagram(bytes).inspect_err(|_| {
            self.status.parse_errors += 1;
        })?;
        self.arrivals
            .push_back((arrival_time, parsed.header.packet_id));
        let disposition = self.classify(&parsed);
        Ok((parsed, disposition))
    }

    pub fn status(&self, now_us: u64) -> IngestStatus {
        let mut status = self.status.clone();
        for &(at, id) in &self.arrivals {
            if at <= now_us && now_us - at < 1_000_000 {
                status.packet_rates_hz[id as usize] += 1.0;
            }
        }
        status.packet_rate_hz = [0, 2, 6, 7, 13, 16]
            .into_iter()
            .map(|id| status.packet_rates_hz[id])
            .fold(0.0, f64::max);
        status
    }

    /// Close on collector exit; silence never calls this. Returns the identity
    /// so the caller can coordinate recorder shutdown without reopening it.
    pub fn close_current(&mut self) -> Option<SessionIdentity> {
        let identity = self.active.take()?;
        self.closed.insert(identity);
        self.last_frame.clear();
        self.ended = false;
        Some(identity)
    }

    pub fn classify(&mut self, parsed: &ParsedDatagram) -> Disposition {
        let h = &parsed.header;
        if h.session_uid == 0 {
            self.status.uid_zero += 1;
            return Disposition::UidZero;
        }
        let identity = SessionIdentity {
            session_uid: h.session_uid,
            packet_format: h.packet_format,
        };
        if self.closed.contains(&identity) {
            self.status.closed_uid += 1;
            return Disposition::Closed;
        }
        if self.active != Some(identity) {
            self.close_current();
            self.active = Some(identity);
        }
        if matches!(&parsed.packet, Packet::FinalClassification(_)) {
            self.close_current();
            return Disposition::Apply;
        }
        if self.ended {
            return if matches!(&parsed.packet, Packet::SessionHistory(_)) {
                Disposition::Apply
            } else {
                self.status.ignored_ended += 1;
                Disposition::IgnoreEnded
            };
        }
        if let Packet::Event(event) = &parsed.packet {
            if event.details == EventDetails::SessionEnded {
                self.ended = true;
            }
            return Disposition::Apply;
        }
        if self
            .last_frame
            .get(&h.packet_id)
            .is_some_and(|last| h.overall_frame_identifier <= *last)
        {
            self.status.dropped_frames += 1;
            return Disposition::DropFrame;
        }
        self.last_frame
            .insert(h.packet_id, h.overall_frame_identifier);
        Disposition::Apply
    }
}
