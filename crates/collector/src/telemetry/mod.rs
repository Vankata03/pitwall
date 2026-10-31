//! Packed F1 telemetry decoded into format-independent model inputs.
//!
//! Arrays have 24 slots; slots absent from the wire are `None`. A present slot
//! is not necessarily an active car. IDs are u16, G-force is in G, engine
//! temperature is u16 Celsius, and wheel arrays retain RL, RR, FL, FR order.
//! Fields only available in one format are `Option`, never invented zeroes.
//! Packet codes, enum values and fixed UTF-8 name bytes retain their wire values.
//! Counts, indices and floating-point values retain their wire values. Consumers
//! must validate them before indexing arrays or doing domain arithmetic.
//! This module performs no session lifecycle, frame ordering or restriction logic.
mod events;
mod generated;
pub use events::{Event, EventDetails};
pub use generated::*;

pub const MAX_CARS: usize = 24;
pub const HEADER_SIZE: usize = 29;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDatagram {
    pub header: PacketHeader,
    pub packet: Packet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Truncated {
        offset: usize,
        needed: usize,
        actual: usize,
    },
    UnsupportedFormat(u16),
    UnsupportedPacketId {
        format: u16,
        id: u8,
    },
    UnsupportedPacketVersion {
        id: u8,
        version: u8,
    },
    InvalidLength {
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "telemetry parse error: {self:?}")
    }
}
impl std::error::Error for ParseError {}

/// Select a layout for each datagram. Unsupported formats carry the received
/// format in the error so connection status can report setup guidance.
pub fn parse_datagram(bytes: &[u8]) -> Result<ParsedDatagram, ParseError> {
    let mut reader = Reader { bytes, offset: 0 };
    let format = reader.u16()?;
    if !matches!(format, 2025 | 2026) {
        return Err(ParseError::UnsupportedFormat(format));
    }
    reader.offset = 0;
    let header = PacketHeader::read(&mut reader, format)?;
    let expected = generated::packet_size(format, header.packet_id).ok_or(
        ParseError::UnsupportedPacketId {
            format,
            id: header.packet_id,
        },
    )?;
    if header.packet_version != 1 {
        return Err(ParseError::UnsupportedPacketVersion {
            id: header.packet_id,
            version: header.packet_version,
        });
    }
    if bytes.len() != expected {
        return Err(ParseError::InvalidLength {
            expected,
            actual: bytes.len(),
        });
    }
    let packet = generated::read_packet(&mut reader, format, header.packet_id)?;
    debug_assert_eq!(reader.offset, bytes.len());
    Ok(ParsedDatagram { header, packet })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ParseError> {
        let end = self.offset + N;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ParseError::Truncated {
                offset: self.offset,
                needed: N,
                actual: self.bytes.len().saturating_sub(self.offset),
            })?;
        self.offset = end;
        Ok(slice.try_into().expect("checked length"))
    }
    fn array<T, const N: usize>(
        &mut self,
        mut read: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Result<[T; N], ParseError> {
        let mut values = [const { None }; N];
        for value in &mut values {
            *value = Some(read(self)?);
        }
        Ok(values.map(|v| v.expect("all elements read")))
    }
    fn cars<T>(
        &mut self,
        count: usize,
        mut read: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Result<[Option<T>; MAX_CARS], ParseError> {
        let mut values = [const { None }; MAX_CARS];
        for value in &mut values[..count] {
            *value = Some(read(self)?);
        }
        Ok(values)
    }
}
macro_rules! read_numbers {
    ($($kind:ident),*) => { impl Reader<'_> { $(fn $kind(&mut self) -> Result<$kind, ParseError> { Ok($kind::from_le_bytes(self.take()?)) })* } };
}
read_numbers!(u8, i8, u16, i16, u32, u64, f32, f64);
