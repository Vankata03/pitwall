//! Per-datagram ingest gate shared by live collection, recording, and the
//! session model. The raw tap runs before any packet can be dropped.
use crate::telemetry::{EventDetails, Packet, ParsedDatagram};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    UidZero,
    Apply,
    DropFrame,
    IgnoreEnded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity {
    pub session_uid: u64,
    pub packet_format: u16,
}

/// Receives every input before UID and frame filtering. Implement this for the
/// recorder; its input is the original datagram bytes and arrival timestamp.
pub trait DatagramTap {
    fn datagram(&mut self, arrival_time: u64, bytes: &[u8]);
}

#[derive(Default)]
pub struct Ingest {
    last_frame: HashMap<(u64, u16, u8), u32>,
    ended: std::collections::HashSet<(u64, u16)>,
}

impl Ingest {
    /// Tap raw bytes first, then parse/filter. A successful Apply result is the
    /// hand-off to the session model (#31); UID zero never reaches that model.
    pub fn ingest<T: DatagramTap>(
        &mut self,
        arrival_time: u64,
        bytes: &[u8],
        tap: &mut T,
    ) -> Result<(ParsedDatagram, Disposition), crate::telemetry::ParseError> {
        tap.datagram(arrival_time, bytes);
        let parsed = crate::telemetry::parse_datagram(bytes)?;
        let disposition = self.classify(&parsed);
        Ok((parsed, disposition))
    }

    pub fn classify(&mut self, parsed: &ParsedDatagram) -> Disposition {
        let h = &parsed.header;
        if h.session_uid == 0 {
            return Disposition::UidZero;
        }
        let identity = (h.session_uid, h.packet_format);
        if matches!(&parsed.packet, Packet::Event(e) if e.details == EventDetails::SessionEnded) {
            self.ended.insert(identity);
            return Disposition::Apply;
        }
        if matches!(&parsed.packet, Packet::FinalClassification(_)) {
            return Disposition::Apply;
        }
        if self.ended.contains(&identity) {
            return if matches!(&parsed.packet, Packet::SessionHistory(_)) {
                Disposition::Apply
            } else {
                Disposition::IgnoreEnded
            };
        }
        let key = (h.session_uid, h.packet_format, h.packet_id);
        if let Some(last) = self.last_frame.get(&key) {
            if h.overall_frame_identifier <= *last {
                return Disposition::DropFrame;
            }
        }
        self.last_frame.insert(key, h.overall_frame_identifier);
        Disposition::Apply
    }

    pub fn reset_session(&mut self, identity: SessionIdentity) {
        self.last_frame.retain(|(uid, format, _), _| {
            (*uid, *format) != (identity.session_uid, identity.packet_format)
        });
        self.ended.remove(&(identity.session_uid, identity.packet_format));
    }
}
