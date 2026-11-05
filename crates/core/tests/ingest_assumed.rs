//! Assumed synthetic ingest tests; packet handling still needs capture proof.
use pitwall_core::{
    ingest::{DatagramTap, Disposition, Ingest},
    telemetry::parse_datagram,
};
use std::process::Command;

fn assumed(id: u8, frame: u32, uid: u64, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new(if cfg!(windows) { "python" } else { "python3" });
    cmd.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ))
    .args(["2025".to_owned(), id.to_string()]);
    for (field, value) in [
        ("header.session_uid", uid.to_string()),
        ("header.overall_frame_identifier", frame.to_string()),
    ] {
        cmd.args(["--set", &format!("{field}={value}")]);
    }
    for (field, value) in extra {
        cmd.args(["--set", &format!("{field}={value}")]);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[derive(Default)]
struct Tap(Vec<(u64, Vec<u8>)>);
impl DatagramTap for Tap {
    fn datagram(&mut self, at: u64, bytes: &[u8]) {
        self.0.push((at, bytes.to_vec()));
    }
}

#[test]
fn assumed_raw_tap_sees_datagram_before_duplicate_frame_drop() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    let bytes = assumed(6, 5, 10, &[]);
    assert_eq!(
        gate.ingest(1, &bytes, &mut tap).unwrap().1,
        Disposition::Apply
    );
    assert_eq!(
        gate.ingest(2, &bytes, &mut tap).unwrap().1,
        Disposition::DropFrame
    );
    assert_eq!(tap.0, vec![(1, bytes.clone()), (2, bytes)]);
}

#[test]
fn assumed_frame_order_is_per_packet_and_events_apply_in_arrival_order() {
    let mut gate = Ingest::default();
    for (id, frame, expected) in [
        (6, 9, Disposition::Apply),
        (7, 8, Disposition::Apply),
        (6, 9, Disposition::DropFrame),
        (3, 1, Disposition::Apply),
        (3, 1, Disposition::Apply),
    ] {
        let parsed = parse_datagram(&assumed(id, frame, 11, &[])).unwrap();
        assert_eq!(gate.classify(&parsed), expected);
    }
}

#[test]
fn assumed_uid_zero_and_ended_session_exemptions() {
    let mut gate = Ingest::default();
    let uid_zero = parse_datagram(&assumed(6, 0, 0, &[])).unwrap();
    assert_eq!(gate.classify(&uid_zero), Disposition::UidZero);
    let send = parse_datagram(&assumed(
        3,
        100,
        12,
        &[
            ("event_string_code.0", "83"),
            ("event_string_code.1", "69"),
            ("event_string_code.2", "78"),
            ("event_string_code.3", "68"),
        ],
    ))
    .unwrap();
    assert_eq!(gate.classify(&send), Disposition::Apply);
    assert_eq!(
        gate.classify(&parse_datagram(&assumed(6, 101, 12, &[])).unwrap()),
        Disposition::IgnoreEnded
    );
    assert_eq!(
        gate.classify(&parse_datagram(&assumed(11, 0, 12, &[])).unwrap()),
        Disposition::Apply
    );
    assert_eq!(
        gate.classify(&parse_datagram(&assumed(8, 0, 12, &[])).unwrap()),
        Disposition::Apply
    );
}

#[test]
fn assumed_events_with_decreasing_frames_always_apply() {
    let mut gate = Ingest::default();
    for frame in [10, 9, 10, 0] {
        let parsed = parse_datagram(&assumed(3, frame, 11, &[])).unwrap();
        assert_eq!(gate.classify(&parsed), Disposition::Apply);
    }
}

#[test]
fn assumed_new_identity_resets_frames_and_closed_identities_never_reopen() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    for (uid, frame, expected) in [
        (20, 900, Disposition::Apply),
        (21, 0, Disposition::Apply),
        (20, 901, Disposition::Closed),
        (21, 1, Disposition::Apply),
    ] {
        assert_eq!(
            gate.ingest(0, &assumed(6, frame, uid, &[]), &mut tap)
                .unwrap()
                .1,
            expected
        );
    }
    assert_eq!(gate.status(0).closed_uid, 1);
    assert_eq!(tap.0.len(), 4, "recorder owns its boundary filtering");
}

#[test]
fn assumed_format_change_is_new_session_and_final_classification_closes() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    let old = assumed(6, 999, 30, &[]);
    gate.ingest(0, &old, &mut tap).unwrap();
    let mut new = assumed(3, 0, 30, &[]);
    new[..2].copy_from_slice(&2026u16.to_le_bytes());
    assert_eq!(
        gate.ingest(1, &new, &mut tap).unwrap().1,
        Disposition::Apply
    );
    assert_eq!(
        gate.ingest(2, &old, &mut tap).unwrap().1,
        Disposition::Closed
    );
    let mut final_classification = assumed(8, 0, 30, &[]);
    // Use 2025 under a different UID, then verify frame zero always applies.
    final_classification[7..15].copy_from_slice(&31u64.to_le_bytes());
    assert_eq!(
        gate.ingest(3, &final_classification, &mut tap).unwrap().1,
        Disposition::Apply
    );
    assert_eq!(
        gate.ingest(4, &final_classification, &mut tap).unwrap().1,
        Disposition::Closed
    );
}

#[test]
fn assumed_status_counts_rejected_inputs_and_rate_decays_during_silence() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    let bytes = assumed(6, 1, 40, &[]);
    for n in 0..60 {
        gate.ingest(n * 16_667, &bytes, &mut tap).unwrap();
    }
    gate.ingest(999_999, &assumed(6, 0, 0, &[]), &mut tap)
        .unwrap();
    assert!(
        gate.ingest(999_999, &2024u16.to_le_bytes(), &mut tap)
            .is_err()
    );
    let status = gate.status(999_999);
    assert_eq!(status.received, 62);
    assert_eq!(status.dropped_frames, 59);
    assert_eq!(status.uid_zero, 1);
    assert_eq!(status.parse_errors, 1);
    assert_eq!(status.last_format, Some(2024));
    assert_eq!(status.packet_rates_hz[6], 61.0);
    assert_eq!(
        status.packet_rate_hz, 61.0,
        "rate is per type, never aggregate datagrams"
    );
    assert_eq!(gate.status(2_000_000).packet_rate_hz, 0.0);
    assert_eq!(tap.0.len(), 62, "raw tap sees malformed input too");
}

#[test]
fn assumed_frame_zero_flush_is_not_exempt_and_silence_never_resets_frames() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    let history = assumed(11, 100, 60, &[]);
    assert_eq!(
        gate.ingest(0, &history, &mut tap).unwrap().1,
        Disposition::Apply
    );
    assert_eq!(
        gate.ingest(1, &assumed(11, 0, 60, &[]), &mut tap)
            .unwrap()
            .1,
        Disposition::DropFrame
    );
    assert_eq!(gate.status(120_000_000).packet_rate_hz, 0.0);
    assert_eq!(
        gate.ingest(120_000_000, &history, &mut tap).unwrap().1,
        Disposition::DropFrame
    );
    assert_eq!(
        gate.ingest(120_000_001, &assumed(11, 101, 60, &[]), &mut tap)
            .unwrap()
            .1,
        Disposition::Apply
    );
}
