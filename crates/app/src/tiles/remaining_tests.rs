//! SM and Track map read from a model fed synthetic (assumed) packets.

use super::{sm, track_map};
use pitwall_core::{
    model::SessionModel,
    telemetry::{Packet, ParsedDatagram, parse_datagram},
    tracks::{Source, TrackStore},
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

/// Melbourne, car 0 named Alpha in red at world (10, 20), third.
fn session() -> SessionModel {
    let mut model = SessionModel::default();
    let mut session = assumed(1);
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.track_id = 0;
    data.track_length = 5276;
    model.apply(Duration::ZERO, &session);
    // Lap Data with a running result status makes the slot valid.
    let mut lap = assumed(2);
    let Packet::Lap(data) = &mut lap.packet else {
        unreachable!()
    };
    let car = data.lap_data[0].as_mut().unwrap();
    car.car_position = 3;
    car.result_status = 2;
    model.apply(Duration::ZERO, &lap);
    let mut participants = assumed(4);
    let Packet::Participants(data) = &mut participants.packet else {
        unreachable!()
    };
    data.num_active_cars = data.num_active_cars.max(1);
    let alpha = data.participants[0].as_mut().unwrap();
    alpha.your_telemetry = 1;
    alpha.name = [0; 32];
    alpha.name[..5].copy_from_slice(b"Alpha");
    alpha.ai_controlled = 1;
    alpha.livery_colour[0].red = 200;
    alpha.livery_colour[0].green = 10;
    alpha.livery_colour[0].blue = 20;
    model.apply(Duration::ZERO, &participants);
    let mut motion = assumed(0);
    let Packet::Motion(data) = &mut motion.packet else {
        unreachable!()
    };
    let car = data.car_motion_data[0].as_mut().unwrap();
    car.world_position_x = 10.0;
    car.world_position_z = 20.0;
    model.apply(Duration::ZERO, &motion);
    model
}

#[test]
fn the_map_reads_positions_team_colours_and_the_shipped_outline_from_the_model() {
    let model = session();
    let dir = std::env::temp_dir().join(format!("pitwall-map-model-{}", std::process::id()));
    let store = TrackStore::open(dir);
    let mut memory = track_map::Memory::default();
    let inputs = memory.inputs(&model, Some(&store), Some(0), (300.0, 240.0));
    let car = inputs.cars.iter().find(|c| c.index == 0).unwrap();
    assert_eq!((car.x, car.z, car.stale), (10.0, 20.0, false));
    assert_eq!(car.position, Some(3));
    assert_eq!(car.colour, Some((200, 10, 20)));
    assert_eq!(inputs.focus_name.as_deref(), Some("ALPHA"));
    let track_map::Lookup::Found(map) = &inputs.map else {
        panic!("Melbourne ships")
    };
    assert_eq!(map.source, Source::Shipped);
    let view = track_map::view(&inputs, &mut memory.fit);
    assert_eq!(view.track_name, "Melbourne");
    assert!(!view.learning);
    assert!(
        view.dots
            .iter()
            .any(|d| d.fill == track_map::Fill::Team(200, 10, 20))
    );
}

fn aero(model: &mut SessionModel, regulations_2026: u8, mode: u8) {
    let mut aero = assumed(16);
    let Packet::CarTelemetry2(data) = &mut aero.packet else {
        unreachable!()
    };
    let car = data.car_telemetry2_data[0].as_mut().unwrap();
    car.regulations_2026 = regulations_2026;
    car.active_aero_mode = mode;
    car.active_aero_available = 0;
    model.apply(Duration::ZERO, &aero);
}

#[test]
fn sm_reads_the_aero_mode_on_2026_cars_and_drs_on_older_ones() {
    let mut model = session();
    let read = |model: &SessionModel| {
        let v = sm::view(&sm::Inputs::from_model(model, Some(0), (220.0, 80.0)));
        (v.label, v.state.text)
    };
    assert_eq!(read(&model).0, "SM / DRS");
    aero(&mut model, 1, 1);
    assert_eq!(read(&model), ("SM", "ACTIVE".into()));
    aero(&mut model, 0, 0);
    let mut telemetry = assumed(6);
    let Packet::CarTelemetry(data) = &mut telemetry.packet else {
        unreachable!()
    };
    data.car_telemetry_data[0].as_mut().unwrap().drs = 1;
    model.apply(Duration::ZERO, &telemetry);
    assert_eq!(read(&model), ("DRS", "OPEN".into()));
}
