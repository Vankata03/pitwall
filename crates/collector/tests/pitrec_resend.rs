//! Synthetic, assumed recordings. Explicit local runs only; no UDP or timing tests in CI.
use pitwall_collector::pitrec::{Header, Writer};
use std::{
    fs,
    net::UdpSocket,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Recording(PathBuf);
impl Recording {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pitrec-resend-assumed-{}-{}.pitrec",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn recording(records: &[(u64, Vec<u8>)]) -> Vec<u8> {
    let header = Header {
        packet_format: 2026,
        game_version: "26.1.0".into(),
        pitwall_version: "assumed".into(),
        started_utc: "2026-11-01T00:00:00Z".into(),
    };
    let mut writer = Writer::new(Vec::new(), &header).unwrap();
    for (time, bytes) in records {
        writer.write(*time, bytes).unwrap();
    }
    writer.finish().unwrap()
}

fn datagram(id: u8, marker: u8) -> Vec<u8> {
    let mut bytes = vec![marker; 32];
    bytes[6] = id;
    bytes
}

#[test]
#[ignore = "manual UDP re-send check; not run in CI"]
fn assumed_cli_sends_unchanged_datagrams_in_sequence_with_recorded_spacing() {
    let records = [
        (0, datagram(6, 10)),
        (150_000, datagram(0, 20)),
        (150_000, datagram(99, 30)),
        (300_000, datagram(6, 40)),
        (1_050_000, datagram(0, 50)), // next compressed block
    ];
    let input = Recording::new(&recording(&records));
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pitrec-resend"))
        .arg(&input.0)
        .arg(receiver.local_addr().unwrap().to_string())
        .spawn()
        .unwrap();
    let mut received = Vec::new();
    for _ in &records {
        let mut bytes = [0; 65535];
        let length = receiver.recv(&mut bytes).unwrap();
        received.push((Instant::now(), bytes[..length].to_vec()));
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(
        received.iter().map(|(_, bytes)| bytes).collect::<Vec<_>>(),
        records.iter().map(|(_, bytes)| bytes).collect::<Vec<_>>()
    );
    // Broad scheduler tolerance: catches unpaced sends without claiming latency qualification.
    for (index, minimum_ms) in [(1, 120), (3, 270), (4, 1020)] {
        let elapsed = received[index].0.duration_since(received[0].0);
        assert!(elapsed >= Duration::from_millis(minimum_ms), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }
    receiver
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    assert!(receiver.recv(&mut [0; 65535]).is_err());
}

#[test]
#[ignore = "manual UDP re-send check; not run in CI"]
fn assumed_cli_reports_incomplete_tail_after_sending_complete_records() {
    let data = datagram(6, 42);
    let mut bytes = recording(&[(0, data.clone())]);
    bytes.extend_from_slice(&[1, 2]);
    let input = Recording::new(&bytes);
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pitrec-resend"))
        .arg(&input.0)
        .arg(receiver.local_addr().unwrap().to_string())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("incomplete tail"));
    let mut received = [0; 65535];
    let length = receiver.recv(&mut received).unwrap();
    assert_eq!(&received[..length], data);
}

#[test]
#[ignore = "manual re-send CLI check; not run in CI"]
fn assumed_cli_rejects_invalid_arguments_files_and_complete_blocks() {
    let valid = Recording::new(&recording(&[]));
    let bad_header = Recording::new(b"not a recording");
    let mut corrupt = recording(&[]);
    corrupt.extend_from_slice(&3u32.to_le_bytes());
    corrupt.extend_from_slice(b"bad");
    let bad_block = Recording::new(&corrupt);
    let missing = valid.0.with_extension("missing");
    for (path, destination) in [
        (&bad_header.0, "127.0.0.1:20777"),
        (&bad_block.0, "127.0.0.1:20777"),
        (&missing, "127.0.0.1:20777"),
        (&valid.0, "127.0.0.1:not-a-port"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pitrec-resend"))
            .arg(path)
            .arg(destination)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("pitrec-resend:"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_pitrec-resend"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}

#[test]
#[ignore = "manual IPv6 UDP re-send check; not run in CI"]
fn assumed_cli_supports_ipv6_destinations() {
    let data = datagram(99, 42);
    let input = Recording::new(&recording(&[(0, data.clone())]));
    let receiver = UdpSocket::bind("[::1]:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pitrec-resend"))
        .arg(&input.0)
        .arg(receiver.local_addr().unwrap().to_string())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let mut received = [0; 65535];
    let length = receiver.recv(&mut received).unwrap();
    assert_eq!(&received[..length], data);
}
