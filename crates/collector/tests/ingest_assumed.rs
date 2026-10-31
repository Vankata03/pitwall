//! Assumed synthetic ingest tests; packet handling still needs capture proof.
use pitwall_collector::{
    ingest::{DatagramTap, Disposition, Ingest},
    telemetry::parse_datagram,
};
use std::process::Command;

fn assumed(id: u8, frame: u32, uid: u64, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new("python3");
    cmd.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/telemetry/assumed_datagram.py"))
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
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    output.stdout
}

#[derive(Default)]
struct Tap(Vec<(u64, Vec<u8>)>);
impl DatagramTap for Tap {
    fn datagram(&mut self, at: u64, bytes: &[u8]) { self.0.push((at, bytes.to_vec())); }
}

#[test]
fn assumed_raw_tap_sees_datagram_before_duplicate_frame_drop() {
    let mut gate = Ingest::default();
    let mut tap = Tap::default();
    let bytes = assumed(6, 5, 10, &[]);
    assert_eq!(gate.ingest(1, &bytes, &mut tap).unwrap().1, Disposition::Apply);
    assert_eq!(gate.ingest(2, &bytes, &mut tap).unwrap().1, Disposition::DropFrame);
    assert_eq!(tap.0, vec![(1, bytes.clone()), (2, bytes)]);
}

#[test]
fn assumed_frame_order_is_per_packet_and_events_apply_in_arrival_order() {
    let mut gate = Ingest::default();
    for (id, frame, expected) in [(6, 9, Disposition::Apply), (7, 8, Disposition::Apply),
        (6, 9, Disposition::DropFrame), (3, 1, Disposition::Apply), (3, 1, Disposition::Apply)] {
        let fields = if id == 3 {
            &[("event_string_code.0", "83"), ("event_string_code.1", "69"),
                ("event_string_code.2", "78"), ("event_string_code.3", "68")][..]
        } else { &[] };
        let parsed = parse_datagram(&assumed(id, frame, 11, fields)).unwrap();
        assert_eq!(gate.classify(&parsed), expected);
    }
}

#[test]
fn assumed_uid_zero_and_ended_session_exemptions() {
    let mut gate = Ingest::default();
    let uid_zero = parse_datagram(&assumed(6, 0, 0, &[])).unwrap();
    assert_eq!(gate.classify(&uid_zero), Disposition::UidZero);
    let send = parse_datagram(&assumed(3, 100, 12, &[("event_string_code.0", "83"),
        ("event_string_code.1", "69"), ("event_string_code.2", "78"),
        ("event_string_code.3", "68")])).unwrap();
    assert_eq!(gate.classify(&send), Disposition::Apply);
    assert_eq!(gate.classify(&parse_datagram(&assumed(6, 101, 12, &[])).unwrap()), Disposition::IgnoreEnded);
    assert_eq!(gate.classify(&parse_datagram(&assumed(11, 0, 12, &[])).unwrap()), Disposition::Apply);
    assert_eq!(gate.classify(&parse_datagram(&assumed(8, 0, 12, &[])).unwrap()), Disposition::Apply);
}
