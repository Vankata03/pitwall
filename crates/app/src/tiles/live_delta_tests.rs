//! Live delta built from a model that drove laps through synthetic Lap Data.

use super::live_delta::*;
use pitwall_core::{
    model::{Readout, SessionModel},
    telemetry::{Packet, ParsedDatagram, parse_datagram},
};
use std::time::Duration;

fn assumed(id: u8) -> ParsedDatagram {
    let output = std::process::Command::new("python")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args([
            "2026",
            &id.to_string(),
            "--set",
            "header.session_uid=101",
            "--set",
            "header.player_car_index=0",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    parse_datagram(&output.stdout).unwrap()
}

/// Laps of a 1000 m track at a pace in ms per metre: lap 1 50 s, lap 2 55 s, lap 3 45 s
/// (the best), then lap 4 at 50 s pace up to `to` metres.
fn drive(to: u32) -> SessionModel {
    let mut model = SessionModel::default();
    let mut session = assumed(1);
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.track_length = 1000;
    data.sector2_lap_distance_start = 300.0;
    data.sector3_lap_distance_start = 700.0;
    model.apply(Duration::ZERO, &session);
    let template = assumed(2);
    let (mut clock, mut last) = (0.0f32, 0u32);
    for (lap, pace, end) in [(1u8, 50, 1000), (2, 55, 1000), (3, 45, 1000), (4, 50, to)] {
        for d in (0..end).step_by(25) {
            let mut packet = template.clone();
            let lap_time = d * pace;
            packet.header.session_time = clock + lap_time as f32 / 1000.0;
            let Packet::Lap(data) = &mut packet.packet else {
                unreachable!()
            };
            for car in data.lap_data.iter_mut().flatten() {
                car.result_status = 2;
            }
            let car = data.lap_data[0].as_mut().unwrap();
            car.current_lap_num = lap;
            car.lap_distance = d as f32;
            car.current_lap_time_in_ms = lap_time;
            car.last_lap_time_in_ms = last;
            car.current_lap_invalid = 0;
            car.pit_status = 0;
            car.sector = 0;
            car.sector1_time_ms_part = 0;
            car.sector2_time_ms_part = 0;
            model.apply(Duration::from_secs_f32(packet.header.session_time), &packet);
        }
        clock += (pace * 1000) as f32 / 1000.0;
        last = pace * 1000;
    }
    model
}

#[test]
fn the_graph_is_this_lap_against_the_reference_and_the_finished_lap_against_its_own() {
    let model = drive(500);
    let best = Inputs::from_model(&model, Some(0), Reference::Best, (600.0, 300.0));
    assert_eq!(best.track_length, 1000.0);
    assert_eq!(best.sectors, [300.0, 700.0]);
    // Lap 4 (50 ms/m) against the best, lap 3 (45 ms/m): 5 ms behind per metre.
    let at = |points: &[Point], d: f32| points.iter().find(|p| p.distance == d).unwrap().delta;
    assert!((at(&best.current, 400.0) - 2.0).abs() < 1e-3);
    // Lap 3 was driven against the best before it, lap 1 (50 ms/m): ahead.
    assert!((at(&best.previous, 800.0) + 4.0).abs() < 1e-3);
    match best.delta {
        Readout::Live(ms) | Readout::Stale(ms) => assert!((ms - 2375.0).abs() < 1.0),
        other => panic!("{other:?}"),
    }
    let previous = Inputs::from_model(&model, Some(0), Reference::Previous, (600.0, 300.0));
    // Lap 3 against lap 2 (55 ms/m): 10 ms ahead per metre.
    assert!((at(&previous.previous, 800.0) + 8.0).abs() < 1e-3);
    assert!((at(&previous.current, 400.0) - 2.0).abs() < 1e-3);

    let v = view(&best);
    let chart = v.chart.unwrap();
    assert!(chart.ahead.iter().flatten().all(|p| p.distance >= 475.0));
    assert_eq!(chart.extent, 5.0);
}

#[test]
fn no_car_and_no_reference_read_absent_without_a_graph() {
    let model = drive(500);
    let none = Inputs::from_model(&model, None, Reference::Best, (600.0, 300.0));
    assert_eq!(none.delta, Readout::Absent);
    assert!(none.current.is_empty() && none.previous.is_empty());
    let other = Inputs::from_model(&model, Some(5), Reference::Best, (600.0, 300.0));
    assert!(other.current.is_empty());
}
