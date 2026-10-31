//! Synthetic session input; assumed until qualified against captures.
use pitwall_collector::{
    model::SessionModel,
    telemetry::{Packet, parse_datagram},
    wire::{Context, State},
};
use std::{process::Command, time::Duration};

#[test]
fn track_map_receives_session_sector_distances() {
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args(["2026", "1", "--set", "header.session_uid=101"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut packet = parse_datagram(&output.stdout).unwrap();
    if let Packet::Session(s) = &mut packet.packet {
        s.sector2_lap_distance_start = 1234.5;
        s.sector3_lap_distance_start = 4567.25;
    } else {
        panic!("expected session");
    }
    let mut model = SessionModel::default();
    model.apply(Duration::ZERO, &packet);
    let wire = serde_json::to_value(State::capture(&model, Context::default())).unwrap();
    assert_eq!(wire["session"]["value"]["sector2LapDistanceStart"], 1234.5);
    assert_eq!(wire["session"]["value"]["sector3LapDistanceStart"], 4567.25);
}
