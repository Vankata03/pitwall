//! Synthetic `.pwcap` and recording fixtures: assumed, not capture evidence.
use pitwall_collector::pitrec::Reader;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pitrec-assumed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn datagram(uid: u64, packet_id: u8) -> Vec<u8> {
    let mut data = vec![0; 29];
    data[..7].copy_from_slice(&[0xea, 0x07, 26, 1, 0, 1, packet_id]);
    data[7..15].copy_from_slice(&uid.to_le_bytes());
    data
}
fn pwcap(records: &[(u64, Vec<u8>)]) -> Vec<u8> {
    let metadata = br#"{"label":"assumed","started_utc":"2026-10-31T00:00:00+00:00","port":20777}"#;
    let mut bytes = b"PWCAP1\n".to_vec();
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(metadata);
    for (ns, data) in records {
        bytes.extend_from_slice(&ns.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u16).to_le_bytes());
        bytes.extend_from_slice(data);
    }
    bytes
}
fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pitrec"))
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn assumed_converter_preserves_raw_datagrams_and_rebases_nanoseconds() {
    let dir = Scratch::new();
    let input = dir.path("assumed.pwcap");
    let output = dir.path("assumed.pitrec");
    let records = [
        (999, datagram(42, 6)),
        (1_001, datagram(42, 0)),
        (1_000_001_999, datagram(42, 99)),
    ];
    let mut capture = pwcap(&records);
    capture.extend_from_slice(&[1, 2, 3]); // interrupted final capture record
    fs::write(&input, capture).unwrap();
    success(run(&[
        "convert".as_ref(),
        input.as_os_str(),
        output.as_os_str(),
    ]));
    let mut reader = Reader::new(fs::File::open(output).unwrap()).unwrap();
    assert_eq!(reader.header().packet_format, 2026);
    assert_eq!(reader.header().game_version, "26.1.0");
    assert_eq!(
        reader.header().started_utc,
        "2026-10-31T00:00:00.000000999Z"
    );
    let actual = reader.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        actual.iter().map(|r| r.arrival_us).collect::<Vec<_>>(),
        [0, 0, 1_000_001]
    );
    assert_eq!(
        actual.into_iter().map(|r| r.datagram).collect::<Vec<_>>(),
        records.into_iter().map(|r| r.1).collect::<Vec<_>>()
    );
}

#[test]
fn assumed_cutter_uses_half_open_microsecond_range_and_rebases_clip() {
    use pitwall_collector::pitrec::{Header, Writer};
    let dir = Scratch::new();
    let input = dir.path("source.pitrec");
    let output = dir.path("clip.pitrec");
    let header = Header {
        packet_format: 2026,
        game_version: "26.1.0".into(),
        pitwall_version: "source-version".into(),
        started_utc: "2026-10-31T00:00:00Z".into(),
    };
    let mut writer = Writer::new(Vec::new(), &header).unwrap();
    for (time, id) in [(0, 0), (1_000_000, 6), (1_000_002, 99), (2_000_000, 0)] {
        writer.write(time, &datagram(42, id)).unwrap();
    }
    fs::write(&input, writer.finish().unwrap()).unwrap();
    success(run(&[
        "clip".as_ref(),
        input.as_os_str(),
        output.as_os_str(),
        "500000".as_ref(),
        "2000000".as_ref(),
    ]));
    let reader = Reader::new(fs::File::open(&output).unwrap()).unwrap();
    assert_eq!(reader.header().started_utc, "2026-10-31T00:00:01Z");
    assert_eq!(reader.header().pitwall_version, "source-version");
    let records = reader.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| (r.seq, r.arrival_us))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 2)]
    );
    assert_eq!(
        records.into_iter().map(|r| r.datagram).collect::<Vec<_>>(),
        [datagram(42, 6), datagram(42, 99)]
    );
}

#[test]
fn assumed_converter_rejects_mixed_sessions_without_leaving_output() {
    let dir = Scratch::new();
    let input = dir.path("mixed.pwcap");
    let output = dir.path("mixed.pitrec");
    fs::write(
        &input,
        pwcap(&[(0, datagram(42, 0)), (1000, datagram(43, 0))]),
    )
    .unwrap();
    let result = run(&["convert".as_ref(), input.as_os_str(), output.as_os_str()]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("multiple session identities"));
    assert!(!output.exists());
}

#[test]
fn assumed_converter_selects_uid_and_format_skipping_zero_and_other_identities() {
    let dir = Scratch::new();
    let input = dir.path("mixed.pwcap");
    let output = dir.path("selected.pitrec");
    let mut older_format = datagram(42, 0);
    older_format[..2].copy_from_slice(&2025u16.to_le_bytes());
    fs::write(
        &input,
        pwcap(&[
            (0, datagram(0, 0)),
            (1000, datagram(43, 0)),
            (2000, older_format),
            (3000, datagram(42, 6)),
            (4000, datagram(42, 99)),
        ]),
    )
    .unwrap();
    success(run(&[
        "convert".as_ref(),
        input.as_os_str(),
        output.as_os_str(),
        "--uid".as_ref(),
        "0x2a".as_ref(),
        "--format".as_ref(),
        "2026".as_ref(),
    ]));
    let reader = Reader::new(fs::File::open(&output).unwrap()).unwrap();
    assert_eq!(reader.header().started_utc, "2026-10-31T00:00:00.000003Z");
    let records = reader.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        records
            .iter()
            .map(|r| (r.seq, r.arrival_us))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 1)]
    );
    assert_eq!(
        records.into_iter().map(|r| r.datagram).collect::<Vec<_>>(),
        [datagram(42, 6), datagram(42, 99)]
    );
}

#[test]
fn assumed_tools_never_overwrite_existing_output_or_the_input() {
    let dir = Scratch::new();
    let input = dir.path("input.pwcap");
    let output = dir.path("existing.pitrec");
    let capture = pwcap(&[(0, datagram(42, 0))]);
    fs::write(&input, &capture).unwrap();
    fs::write(&output, b"keep this file").unwrap();
    assert!(
        !run(&["convert".as_ref(), input.as_os_str(), output.as_os_str()])
            .status
            .success()
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep this file");
    assert!(
        !run(&["convert".as_ref(), input.as_os_str(), input.as_os_str()])
            .status
            .success()
    );
    assert_eq!(fs::read(&input).unwrap(), capture);
}

#[test]
fn assumed_converter_rejects_malformed_or_empty_input_without_leaving_output() {
    let dir = Scratch::new();
    let input = dir.path("bad.pwcap");
    let output = dir.path("bad.pitrec");
    let mut unsupported = datagram(42, 0);
    unsupported[..2].copy_from_slice(&2024u16.to_le_bytes());
    let mut bad_metadata = b"PWCAP1\n".to_vec();
    bad_metadata.extend_from_slice(&u32::MAX.to_le_bytes());
    for bytes in [
        b"not a capture".to_vec(),
        bad_metadata,
        pwcap(&[]),
        pwcap(&[(0, vec![0; 4])]),
        pwcap(&[(0, unsupported)]),
        pwcap(&[(0, datagram(0, 0))]),
        pwcap(&[(999, datagram(42, 0)), (998, datagram(42, 6))]),
    ] {
        fs::write(&input, bytes).unwrap();
        assert!(
            !run(&["convert".as_ref(), input.as_os_str(), output.as_os_str()])
                .status
                .success()
        );
        assert!(!output.exists());
    }
}

#[test]
fn assumed_cutter_rejects_empty_and_reversed_ranges_and_preserves_existing_files() {
    let dir = Scratch::new();
    let input = dir.path("source.pitrec");
    let capture = dir.path("source.pwcap");
    let output = dir.path("clip.pitrec");
    fs::write(&capture, pwcap(&[(0, datagram(42, 0))])).unwrap();
    success(run(&[
        "convert".as_ref(),
        capture.as_os_str(),
        input.as_os_str(),
    ]));
    for (start, end) in [("1", "2"), ("1", "0"), ("0", "0"), ("-1", "10")] {
        assert!(
            !run(&[
                "clip".as_ref(),
                input.as_os_str(),
                output.as_os_str(),
                start.as_ref(),
                end.as_ref()
            ])
            .status
            .success()
        );
        assert!(!output.exists());
    }
    let original = fs::read(&input).unwrap();
    assert!(
        !run(&[
            "clip".as_ref(),
            input.as_os_str(),
            input.as_os_str(),
            "0".as_ref(),
            "1".as_ref()
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&input).unwrap(), original);
}
