//! Append-only `.pitrec` version 1 codec. No telemetry parsing or session lifecycle.
//!
//! Call `Writer::finish` on close; dropping it discards its unfinished block.
//! `flush_block` makes buffered records recoverable, without closing the stream.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{self, Read, Write};

pub const MAGIC: [u8; 8] = *b"PITREC\x01\x00";
pub const MAX_HEADER_BYTES: usize = 1024 * 1024;
pub const MAX_BLOCK_BYTES: usize = 64 * 1024 * 1024;
/// Fast compression for recording; final recompression policy belongs to the recorder.
pub const COMPRESSION_LEVEL: i32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub packet_format: u16,
    pub game_version: String,
    pub pitwall_version: String,
    /// RFC 3339 UTC start time, supplied by the caller (the codec reads no clock).
    pub started_utc: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u32,
    pub arrival_us: u64,
    pub datagram: Vec<u8>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct Writer<W: Write> {
    output: W,
    records: Vec<Record>,
    origin: Option<u64>,
    last_time: u64,
    next_seq: u64,
    buffered_bytes: usize,
    level: i32,
}

impl<W: Write> Writer<W> {
    pub fn new(mut output: W, header: &Header) -> io::Result<Self> {
        let json = serde_json::to_vec(header)?;
        if json.len() > MAX_HEADER_BYTES {
            return Err(invalid("pitrec header exceeds size limit"));
        }
        output.write_all(&MAGIC)?;
        output.write_all(&(json.len() as u32).to_le_bytes())?;
        output.write_all(&json)?;
        Ok(Self {
            output,
            records: Vec::new(),
            origin: None,
            last_time: 0,
            next_seq: 0,
            buffered_bytes: 0,
            level: COMPRESSION_LEVEL,
        })
    }

    /// Level for blocks flushed from now on; the recorder raises it to recompress.
    pub fn set_compression_level(&mut self, level: i32) {
        self.level = level;
    }

    /// Append in arrival order. Times must be monotonic (ties allowed); the first
    /// supplied microsecond timestamp becomes zero in the file. Datagrams stay raw.
    pub fn write(&mut self, arrival_us: u64, datagram: &[u8]) -> io::Result<()> {
        if datagram.len() > u16::MAX as usize {
            return Err(invalid("datagram exceeds u16 length"));
        }
        if self.origin.is_some() && arrival_us < self.last_time {
            return Err(invalid("arrival timestamps must be monotonic"));
        }
        let seq = u32::try_from(self.next_seq).map_err(|_| invalid("pitrec sequence exhausted"))?;
        let origin = *self.origin.get_or_insert(arrival_us);
        let time = arrival_us - origin;
        if self
            .records
            .first()
            .is_some_and(|r| time - r.arrival_us >= 1_000_000)
            || self.buffered_bytes + 14 + datagram.len() > MAX_BLOCK_BYTES
        {
            self.flush_block()?;
        }
        self.records.push(Record {
            seq,
            arrival_us: time,
            datagram: datagram.to_vec(),
        });
        self.buffered_bytes += 14 + datagram.len();
        self.next_seq += 1;
        self.last_time = arrival_us;
        Ok(())
    }

    /// Write one independent zstd frame, grouping records by their packet ID
    /// (byte 6 in both supported telemetry formats). Short raw inputs sort last.
    pub fn flush_block(&mut self) -> io::Result<()> {
        if self.records.is_empty() {
            return Ok(());
        }
        self.records
            .sort_by_key(|r| r.datagram.get(6).copied().unwrap_or(u8::MAX));
        let mut raw = Vec::with_capacity(self.buffered_bytes);
        for r in &self.records {
            raw.extend_from_slice(&r.seq.to_le_bytes());
            raw.extend_from_slice(&r.arrival_us.to_le_bytes());
            raw.extend_from_slice(&(r.datagram.len() as u16).to_le_bytes());
            raw.extend_from_slice(&r.datagram);
        }
        let compressed = zstd::stream::encode_all(raw.as_slice(), self.level)?;
        self.output
            .write_all(&(compressed.len() as u32).to_le_bytes())?;
        self.output.write_all(&compressed)?;
        self.output.flush()?;
        self.records.clear();
        self.buffered_bytes = 0;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush_block()?;
        self.output.flush()?;
        Ok(self.output)
    }
}

/// Streaming reader, buffering at most one block. Complete blocks are emitted in
/// exact `seq` order; an incomplete final length or payload ends iteration.
/// Malformed complete blocks return an error, then iteration ends.
pub struct Reader<R: Read> {
    input: R,
    header: Header,
    ready: VecDeque<Record>,
    next_seq: u64,
    last_time: u64,
    done: bool,
    incomplete: bool,
}

impl<R: Read> Reader<R> {
    pub fn new(mut input: R) -> io::Result<Self> {
        let mut magic = [0; 8];
        input.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(invalid("unsupported pitrec magic or version"));
        }
        let mut length = [0; 4];
        input.read_exact(&mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length > MAX_HEADER_BYTES {
            return Err(invalid("pitrec header exceeds size limit"));
        }
        let mut json = vec![0; length];
        input.read_exact(&mut json)?;
        let header = serde_json::from_slice(&json)?;
        Ok(Self {
            input,
            header,
            ready: VecDeque::new(),
            next_seq: 0,
            last_time: 0,
            done: false,
            incomplete: false,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Available after iteration reaches the truncated tail. A clean EOF is false.
    pub fn has_incomplete_tail(&self) -> bool {
        self.incomplete
    }

    fn load_block(&mut self) -> io::Result<()> {
        let mut length = [0; 4];
        // Distinguish a clean boundary from a partially written length prefix.
        loop {
            match self.input.read(&mut length[..1]) {
                Ok(0) => {
                    self.done = true;
                    return Ok(());
                }
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        if !self.read_tail(&mut length[1..])? {
            return Ok(());
        }
        let length = u32::from_le_bytes(length) as usize;
        if length > MAX_BLOCK_BYTES + 1024 * 1024 {
            return Err(invalid("compressed block exceeds size limit"));
        }
        let mut compressed = vec![0; length];
        if !self.read_tail(&mut compressed)? {
            return Ok(());
        }
        let frame_size = zstd::zstd_safe::find_frame_compressed_size(&compressed)
            .map_err(|_| invalid("invalid zstd frame"))?;
        if frame_size != length {
            return Err(invalid("expected one zstd frame per block"));
        }
        let decoder = zstd::stream::read::Decoder::new(compressed.as_slice())?;
        let mut raw = Vec::new();
        decoder
            .take((MAX_BLOCK_BYTES + 1) as u64)
            .read_to_end(&mut raw)?;
        if raw.len() > MAX_BLOCK_BYTES {
            return Err(invalid("decoded block exceeds size limit"));
        }
        let mut records = Vec::new();
        let mut offset = 0;
        while offset < raw.len() {
            if raw.len() - offset < 14 {
                return Err(invalid("incomplete record in complete block"));
            }
            let seq = u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
            let arrival_us = u64::from_le_bytes(raw[offset + 4..offset + 12].try_into().unwrap());
            let length =
                u16::from_le_bytes(raw[offset + 12..offset + 14].try_into().unwrap()) as usize;
            offset += 14;
            if raw.len() - offset < length {
                return Err(invalid("incomplete datagram in complete block"));
            }
            records.push(Record {
                seq,
                arrival_us,
                datagram: raw[offset..offset + length].to_vec(),
            });
            offset += length;
        }
        records.sort_by_key(|r| r.seq);
        for r in &records {
            if u64::from(r.seq) != self.next_seq {
                return Err(invalid("non-contiguous pitrec sequence"));
            }
            if (self.next_seq == 0 && r.arrival_us != 0) || r.arrival_us < self.last_time {
                return Err(invalid("invalid pitrec arrival timestamp"));
            }
            self.next_seq += 1;
            self.last_time = r.arrival_us;
        }
        self.ready = records.into();
        Ok(())
    }

    fn read_tail(&mut self, bytes: &mut [u8]) -> io::Result<bool> {
        match self.input.read_exact(bytes) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                self.done = true;
                self.incomplete = true;
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}

impl<R: Read> Iterator for Reader<R> {
    type Item = io::Result<Record>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(record) = self.ready.pop_front() {
                return Some(Ok(record));
            }
            if self.done {
                return None;
            }
            if let Err(error) = self.load_block() {
                self.done = true;
                return Some(Err(error));
            }
        }
    }
}
