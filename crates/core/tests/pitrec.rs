//! Synthetic format fixtures: assumed until checked against a real capture.
use pitwall_core::pitrec::{Header, Reader, Writer};
use std::io::Cursor;

fn header() -> Header {
    Header {
        packet_format: 2026,
        game_version: "26.1.0".into(),
        pitwall_version: "0.1.0".into(),
        started_utc: "2026-10-31T00:00:00Z".into(),
    }
}

#[test]
fn assumed_round_trip_restores_arrival_order_across_grouped_blocks() {
    let mut writer = Writer::new(Vec::new(), &header()).unwrap();
    let datagrams = [
        b"\xea\x07\x1a\x01\x00\x01\x06telemetry".to_vec(),
        b"\xea\x07\x1a\x01\x00\x01\x00motion".to_vec(),
        b"\xea\x07\x1a\x01\x00\x01\x06again".to_vec(),
    ];
    for (time, data) in [500, 501, 1_000_500].into_iter().zip(&datagrams) {
        writer.write(time, data).unwrap();
    }
    let bytes = writer.finish().unwrap();
    assert_eq!(&bytes[..8], b"PITREC\x01\x00");
    let mut reader = Reader::new(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.header(), &header());
    let records = reader.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| (r.seq, r.arrival_us))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 1), (2, 1_000_000)]
    );
    assert_eq!(
        records.into_iter().map(|r| r.datagram).collect::<Vec<_>>(),
        datagrams
    );
    assert!(!reader.has_incomplete_tail());
}

fn file_with_blocks(blocks: &[Vec<u8>]) -> Vec<u8> {
    // Independent construction from the spec, rather than the production writer.
    let json = br#"{"packet_format":2026,"game_version":"26.1.0","pitwall_version":"0.1.0","started_utc":"2026-10-31T00:00:00Z"}"#;
    let mut bytes = b"PITREC\x01\x00".to_vec();
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(json);
    for block in blocks {
        bytes.extend_from_slice(&(block.len() as u32).to_le_bytes());
        bytes.extend_from_slice(block);
    }
    bytes
}

#[test]
fn assumed_writer_layout_groups_packet_ids_and_uses_global_sequences() {
    let mut writer = Writer::new(Vec::new(), &header()).unwrap();
    writer.write(99, b"abcdef\x06").unwrap();
    writer.write(100, b"abcdef\x00").unwrap();
    writer.write(1_000_099, b"abcdef\x03").unwrap();
    let bytes = writer.finish().unwrap();
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let offset = 12 + header_len;
    let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let raw = zstd::stream::decode_all(&bytes[offset + 4..offset + 4 + length]).unwrap();
    // Packet 0 arrives second but is stored first. Literal record layout is
    // seq(u32), arrival_us(u64), length(u16), seven unchanged datagram bytes.
    assert_eq!(
        raw,
        b"\x01\0\0\0\x01\0\0\0\0\0\0\0\x07\0abcdef\0\0\0\0\0\0\0\0\0\0\0\0\0\x07\0abcdef\x06"
    );
    let offset = offset + 4 + length;
    let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let raw = zstd::stream::decode_all(&bytes[offset + 4..offset + 4 + length]).unwrap();
    assert_eq!(raw, b"\x02\0\0\0\x40\x42\x0f\0\0\0\0\0\x07\0abcdef\x03");
    assert_eq!(offset + 4 + length, bytes.len());
}

#[test]
fn assumed_every_incomplete_tail_preserves_only_preceding_complete_blocks() {
    let first = zstd::stream::encode_all(b"\0\0\0\0\0\0\0\0\0\0\0\0\x01\0A".as_slice(), 1).unwrap();
    let second =
        zstd::stream::encode_all(b"\x01\0\0\0\x40\x42\x0f\0\0\0\0\0\x01\0B".as_slice(), 1).unwrap();
    let boundary = file_with_blocks(std::slice::from_ref(&first)).len();
    let bytes = file_with_blocks(&[first, second]);
    for end in boundary..bytes.len() {
        let mut reader = Reader::new(&bytes[..end]).unwrap();
        let records = reader.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(records.len(), 1, "cut at {end}");
        assert_eq!(records[0].datagram, b"A");
        assert_eq!(reader.has_incomplete_tail(), end != boundary);
    }
}

#[test]
fn assumed_reader_rejects_multiple_zstd_frames_in_one_block() {
    let mut block =
        zstd::stream::encode_all(b"\0\0\0\0\0\0\0\0\0\0\0\0\x01\0A".as_slice(), 1).unwrap();
    block.extend(
        zstd::stream::encode_all(b"\x01\0\0\0\x01\0\0\0\0\0\0\0\x01\0B".as_slice(), 1).unwrap(),
    );
    let bytes = file_with_blocks(&[block]);
    let mut reader = Reader::new(bytes.as_slice()).unwrap();
    assert!(reader.next().unwrap().is_err());
    assert!(reader.next().is_none());
}

#[test]
fn assumed_reader_rejects_malformed_complete_blocks_atomically() {
    let cases: &[&[u8]] = &[
        b"\0\0\0", // incomplete record prefix
        b"\0\0\0\0\0\0\0\0\0\0\0\0\x02\0A", // short datagram
        b"\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0A", // sequence gap
        b"\0\0\0\0\x01\0\0\0\0\0\0\0\x01\0A", // non-zero first arrival
        b"\0\0\0\0\0\0\0\0\0\0\0\0\x01\0A\0\0\0\0\0\0\0\0\0\0\0\0\x01\0B", // duplicate
        b"\0\0\0\0\0\0\0\0\0\0\0\0\x01\0A\x01\0\0\0\x02\0\0\0\0\0\0\0\x01\0B\x02\0\0\0\x01\0\0\0\0\0\0\0\x01\0C", // backwards time
    ];
    for raw in cases {
        let block = zstd::stream::encode_all(*raw, 1).unwrap();
        let bytes = file_with_blocks(&[block]);
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        assert!(reader.next().unwrap().is_err(), "accepted {raw:?}");
        assert!(reader.next().is_none());
        assert!(!reader.has_incomplete_tail());
    }
    let bytes = file_with_blocks(&[b"not zstd".to_vec()]);
    assert!(
        Reader::new(bytes.as_slice())
            .unwrap()
            .next()
            .unwrap()
            .is_err()
    );
}

#[test]
fn assumed_explicit_flush_is_append_only_and_rejected_inputs_consume_no_sequence() {
    let mut writer = Writer::new(Vec::new(), &header()).unwrap();
    writer.write(10, b"A").unwrap();
    assert!(writer.write(9, b"backwards").is_err());
    assert!(writer.write(10, &vec![0; 65_536]).is_err());
    writer.flush_block().unwrap();
    writer.flush_block().unwrap();
    writer.write(10, b"B").unwrap();
    let bytes = writer.finish().unwrap();
    let records = Reader::new(bytes.as_slice())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| (r.seq, r.arrival_us))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0)]
    );
    assert_eq!(
        records.into_iter().map(|r| r.datagram).collect::<Vec<_>>(),
        [b"A", b"B"]
    );
}

#[test]
fn assumed_reader_rejects_unsupported_version_truncated_header_and_oversized_lengths() {
    let mut bytes = file_with_blocks(&[]);
    bytes[6] = 2;
    assert!(Reader::new(bytes.as_slice()).is_err());
    for end in 0..file_with_blocks(&[]).len() {
        assert!(Reader::new(&file_with_blocks(&[])[..end]).is_err());
    }
    let mut bytes = b"PITREC\x01\0".to_vec();
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(Reader::new(bytes.as_slice()).is_err());
    let mut bytes = file_with_blocks(&[]);
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        Reader::new(bytes.as_slice())
            .unwrap()
            .next()
            .unwrap()
            .is_err()
    );
}
