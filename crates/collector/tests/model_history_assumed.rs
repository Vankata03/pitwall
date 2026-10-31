//! Assumed synthetic packet-table fixtures, not evidence from game captures.
use pitwall_collector::{
    model::{RaceControlKind, Readout, SessionModel, Source},
    telemetry::{Event, EventDetails, Packet, ParsedDatagram, parse_datagram},
};
use std::{process::Command, sync::Arc, time::Duration};

fn packet(format: u16, id: u8, time: f32) -> ParsedDatagram {
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args([
            format.to_string(),
            id.to_string(),
            "--set".into(),
            "header.session_uid=101".into(),
        ])
        .output()
        .expect("Python is required for assumed fixtures");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut packet = parse_datagram(&output.stdout).unwrap();
    packet.header.session_time = time;
    packet
}

fn apply(model: &mut SessionModel, packet: &ParsedDatagram) {
    model.apply(Duration::from_secs_f32(packet.header.session_time), packet);
}

fn lap(time: f32, number: u8, last_ms: u32, invalid: u8) -> ParsedDatagram {
    let mut packet = packet(2026, 2, time);
    let Packet::Lap(data) = &mut packet.packet else {
        unreachable!()
    };
    for value in data.lap_data.iter_mut().flatten() {
        value.result_status = 2;
        value.current_lap_num = number;
        value.current_lap_time_in_ms = 100;
        value.last_lap_time_in_ms = last_ms;
        value.current_lap_invalid = invalid;
    }
    packet
}

fn event(time: f32, details: EventDetails) -> ParsedDatagram {
    let mut packet = packet(2026, 3, time);
    packet.packet = Packet::Event(Box::new(Event {
        code: *b"TEST",
        details,
        raw_details: [0; 12],
    }));
    packet
}

fn status(time: f32, compound: u8, age: u8) -> ParsedDatagram {
    let mut packet = packet(2026, 7, time);
    if let Packet::CarStatus(data) = &mut packet.packet {
        for value in data.car_status_data.iter_mut().flatten() {
            value.actual_tyre_compound = compound;
            value.visual_tyre_compound = compound;
            value.tyres_age_laps = age;
        }
    }
    packet
}

#[test]
fn assumed_gaps_use_game_race_deltas_distance_and_session_best_in_other_sessions() {
    let mut model = SessionModel::default();
    let mut session = packet(2026, 1, 0.0);
    if let Packet::Session(data) = &mut session.packet {
        data.session_type = 15;
        data.track_length = 5000;
    }
    apply(&mut model, &session);
    let mut positions = lap(1.0, 4, 0, 0);
    if let Packet::Lap(data) = &mut positions.packet {
        for (index, value) in data.lap_data.iter_mut().enumerate() {
            let value = value.as_mut().unwrap();
            value.car_position = index as u8 + 1;
            value.total_distance = 20_100.0;
        }
        let value = data.lap_data[23].as_mut().unwrap();
        value.total_distance = 10_000.0;
        value.delta_to_race_leader_minutes_part = 1;
        value.delta_to_race_leader_ms_part = 12_345;
        value.delta_to_car_in_front_ms_part = 1500;
    }
    apply(&mut model, &positions);
    assert!(matches!(model.car(23).unwrap().gaps(), Readout::Live(g) if
        g.to_leader_ms == 72_345 && g.to_car_in_front_ms == Some(1500) && g.laps_behind == Some(2)));
    model.advance(Duration::from_secs(2));
    assert!(matches!(model.car(23).unwrap().gaps(), Readout::Stale(_)));
    // A car ahead of the leader by track position must not acquire a negative lap count.
    if let Packet::Lap(data) = &mut positions.packet {
        data.lap_data[23].as_mut().unwrap().total_distance = 20_200.0;
    }
    positions.header.session_time = 3.0;
    apply(&mut model, &positions);
    assert!(matches!(model.car(23).unwrap().gaps(), Readout::Live(g) if g.laps_behind == Some(0)));

    for session_type in [1, 5, 18] {
        // Practice, qualifying, Time Trial.
        let mut model = SessionModel::default();
        if let Packet::Session(data) = &mut session.packet {
            data.session_type = session_type;
        }
        apply(&mut model, &session);
        apply(&mut model, &lap(1.0, 1, 0, 0));
        assert!(matches!(model.car(23).unwrap().gaps(), Readout::Absent));
        let mut finish = lap(90.1, 2, 90_000, 0);
        if let Packet::Lap(data) = &mut finish.packet {
            data.lap_data[23].as_mut().unwrap().last_lap_time_in_ms = 92_000;
        }
        apply(&mut model, &finish);
        assert!(matches!(model.car(23).unwrap().gaps(), Readout::Live(g) if
            g.to_leader_ms == 2000 && g.to_car_in_front_ms.is_none() && g.laps_behind.is_none()));
    }
}

#[test]
fn assumed_derived_ers_and_condition_preserve_absent_stale_and_restricted_states() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    assert!(matches!(
        model.car(23).unwrap().tyre_condition(),
        Readout::Absent
    ));
    assert!(matches!(
        model.car(23).unwrap().ers_percentage(),
        Readout::Absent
    ));
    let mut participants = packet(2026, 4, 0.0);
    if let Packet::Participants(data) = &mut participants.packet {
        data.num_active_cars = 24;
        data.participants[23].as_mut().unwrap().your_telemetry = 1;
    }
    apply(&mut model, &participants);
    let mut status = status(0.0, 16, 0);
    if let Packet::CarStatus(data) = &mut status.packet {
        data.car_status_data[23].as_mut().unwrap().ers_store_energy = 2_000_000.0;
    }
    apply(&mut model, &status);
    let mut damage = packet(2026, 10, 0.0);
    if let Packet::CarDamage(data) = &mut damage.packet {
        data.car_damage_data[23].as_mut().unwrap().tyres_wear = [20.0, 30.0, 40.0, 50.0];
    }
    apply(&mut model, &damage);
    assert_eq!(model.car(23).unwrap().ers_percentage(), Readout::Live(50.0));
    assert_eq!(
        model.car(23).unwrap().tyre_condition(),
        Readout::Live([80.0, 70.0, 60.0, 50.0])
    );
    model.advance(Duration::from_secs(1));
    assert_eq!(
        model.car(23).unwrap().ers_percentage(),
        Readout::Stale(50.0)
    );
    assert_eq!(
        model.car(23).unwrap().tyre_condition(),
        Readout::Stale([80.0, 70.0, 60.0, 50.0])
    );
    if let Packet::Participants(data) = &mut participants.packet {
        data.participants[23].as_mut().unwrap().your_telemetry = 0;
    }
    apply(&mut model, &participants);
    assert_eq!(model.car(23).unwrap().ers_percentage(), Readout::Restricted);
    assert_eq!(model.car(23).unwrap().tyre_condition(), Readout::Restricted);
}

#[test]
fn assumed_race_control_keeps_game_events_derived_changes_and_authoritative_penalty_state() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    apply(&mut model, &status(0.0, 16, 0));
    apply(&mut model, &packet(2026, 1, 0.0));
    let mut changed = status(10.0, 16, 0);
    if let Packet::CarStatus(data) = &mut changed.packet {
        data.car_status_data[23].as_mut().unwrap().vehicle_fia_flags = 3;
    }
    apply(&mut model, &changed);
    let mut weather = packet(2026, 1, 11.0);
    if let Packet::Session(data) = &mut weather.packet {
        data.weather = 3;
        data.safety_car_status = 2;
        data.num_marshal_zones = 1;
        data.marshal_zones[0].zone_flag = 3;
    }
    apply(&mut model, &weather);
    let mut pit = lap(12.0, 1, 0, 0);
    if let Packet::Lap(data) = &mut pit.packet {
        data.lap_data[23].as_mut().unwrap().pit_status = 1;
    }
    apply(&mut model, &pit);
    apply(&mut model, &lap(13.0, 1, 0, 0));
    apply(
        &mut model,
        &event(
            14.0,
            EventDetails::Penalty {
                penalty_type: 4,
                infringement_type: 7,
                vehicle_idx: 23,
                other_vehicle_idx: 255,
                time: 5,
                lap_num: 1,
                places_gained: 0,
            },
        ),
    );
    apply(
        &mut model,
        &event(
            15.0,
            EventDetails::Collision {
                vehicle1_idx: 23,
                vehicle2_idx: 0,
                severity: Some(2),
            },
        ),
    );
    let entries = model.race_control();
    assert!(entries.iter().any(|entry| entry.source == Source::Derived
        && entry.cars == [23]
        && matches!(entry.kind, RaceControlKind::FlagChanged { flag: 3, .. })));
    assert!(entries.iter().any(|entry| matches!(
        entry.kind,
        RaceControlKind::WeatherChanged { weather: 3, .. }
    )));
    assert!(entries.iter().any(|entry| matches!(
        entry.kind,
        RaceControlKind::SafetyCarChanged { status: 2, .. }
    )));
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry.kind, RaceControlKind::PitEntry))
    );
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry.kind, RaceControlKind::PitExit))
    );
    assert!(entries.iter().any(|entry| entry.source == Source::Game
        && entry.cars == [23, 0]
        && entry.lap_number == Some(1)));
    assert!(
        entries
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id && pair[0].session_time <= pair[1].session_time)
    );
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.penalties.len(), 1);
    assert_eq!(history.penalties[0].time_seconds, 5);
    assert_eq!(history.penalties[0].session_time, 14.0);
    let mut outstanding = lap(16.0, 1, 0, 0);
    if let Packet::Lap(data) = &mut outstanding.packet {
        let value = data.lap_data[23].as_mut().unwrap();
        value.penalties = 0;
        value.total_warnings = 3;
        value.corner_cutting_warnings = 2;
        value.num_unserved_drive_through_pens = 1;
        value.num_unserved_stop_go_pens = 2;
    }
    apply(&mut model, &outstanding);
    assert!(
        matches!(model.car(23).unwrap().penalty_state(), Readout::Live(p) if p.seconds == 0 && p.total_warnings == 3 && p.corner_cutting_warnings == 2 && p.drive_throughs == 1 && p.stop_gos == 2)
    );
    assert_eq!(model.car(23).unwrap().session_history().penalties.len(), 1);
}

#[test]
fn assumed_flashback_truncates_facts_recomputes_bests_and_keeps_rewound_audit_entries() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    apply(&mut model, &status(1.0, 16, 0));
    apply(&mut model, &lap(90.1, 2, 90_000, 0));
    apply(&mut model, &lap(179.1, 3, 89_000, 0));
    apply(&mut model, &status(180.0, 17, 0));
    let mut pit = lap(181.0, 3, 89_000, 0);
    if let Packet::Lap(data) = &mut pit.packet {
        data.lap_data[23].as_mut().unwrap().pit_status = 2;
        data.lap_data[23].as_mut().unwrap().sector = 1;
        data.lap_data[23].as_mut().unwrap().sector1_time_ms_part = 20_000;
        data.lap_data[23].as_mut().unwrap().current_lap_time_in_ms = 21_000;
    }
    apply(&mut model, &pit);
    apply(&mut model, &lap(182.0, 3, 89_000, 0));
    apply(
        &mut model,
        &event(
            183.0,
            EventDetails::Penalty {
                penalty_type: 4,
                infringement_type: 7,
                vehicle_idx: 23,
                other_vehicle_idx: 255,
                time: 5,
                lap_num: 3,
                places_gained: 0,
            },
        ),
    );
    apply(
        &mut model,
        &event(
            184.0,
            EventDetails::Retirement {
                vehicle_idx: 23,
                reason: 9,
            },
        ),
    );
    let checkpoint = model.clone();
    let epoch = model.epoch();
    let rewind = event(
        184.0,
        EventDetails::Flashback {
            frame_identifier: 100,
            session_time: 100.0,
        },
    );
    model.apply(Duration::from_secs(1000), &rewind);
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.laps.len(), 1);
    assert_eq!(history.best_lap_number, Some(1));
    assert!(
        history
            .sectors
            .iter()
            .all(|sector| sector.session_time <= 100.0)
    );
    assert_eq!(history.stints.len(), 1);
    assert_eq!(history.stints[0].end_lap, None);
    assert!(history.pit_stops.is_empty());
    assert!(history.penalties.is_empty());
    assert!(!model.car(23).unwrap().retired());
    assert!(matches!(model.car(23).unwrap().lap(), Readout::Absent));
    assert_eq!(model.epoch(), epoch + 1);
    assert!(model.race_control().iter().any(|entry| entry.rewound && matches!(&entry.kind, RaceControlKind::Game(game) if matches!(game.details,EventDetails::Penalty {..}))));
    assert_eq!(
        model
            .race_control()
            .iter()
            .filter(|entry| matches!(entry.kind, RaceControlKind::Flashback { .. }))
            .count(),
        1
    );
    assert_eq!(
        checkpoint
            .car(23)
            .unwrap()
            .session_history()
            .best_lap_number,
        Some(2)
    );
    assert!(checkpoint.car(23).unwrap().retired());
    let mut replay = checkpoint.clone();
    replay.apply(Duration::from_secs(1000), &rewind);
    assert_eq!(model, replay);
    model.apply(Duration::from_secs(1001), &lap(101.0, 2, 90_000, 0));
    assert_eq!(model.epoch(), epoch + 1); // FLBK header time must not cause a second rewind.
    assert_eq!(model.car(23).unwrap().session_history().laps.len(), 1);
    model.apply(
        Duration::from_secs(1002),
        &event(
            101.0,
            EventDetails::Flashback {
                frame_identifier: 20,
                session_time: 20.0,
            },
        ),
    );
    assert!(model.car(23).unwrap().session_history().laps.is_empty());
    assert_eq!(
        model.car(23).unwrap().session_history().best_lap_number,
        None
    );
    let mut new_session = lap(0.0, 1, 0, 0);
    new_session.header.session_uid = 102;
    let epoch = model.epoch();
    model.apply(Duration::from_secs(1003), &new_session);
    assert_eq!(model.epoch(), epoch + 1);
    assert!(model.race_control().is_empty());
    assert!(model.car(23).unwrap().session_history().stints.is_empty());
}

#[test]
fn assumed_backwards_session_time_also_rewinds_but_late_history_does_not() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    apply(&mut model, &lap(90.1, 2, 90_000, 0));
    apply(&mut model, &lap(179.1, 3, 89_000, 0));
    let epoch = model.epoch();
    model.apply(Duration::from_secs(200), &lap(100.0, 2, 90_000, 0));
    assert_eq!(model.epoch(), epoch + 1);
    assert_eq!(
        model.car(23).unwrap().session_history().best_lap_number,
        Some(1)
    );
    model.apply(Duration::from_secs(201), &packet(2026, 11, 0.0));
    assert_eq!(model.epoch(), epoch + 1);
}

#[test]
fn assumed_session_history_with_nonzero_partial_lap_time_does_not_complete_the_lap() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    let mut partial = packet(2026, 11, 40.0);
    if let Packet::SessionHistory(data) = &mut partial.packet {
        data.car_idx = 23;
        data.num_laps = 1;
        data.lap_history_data[0].lap_time_in_ms = 40_000;
        data.lap_history_data[0].sector1_time_ms_part = 30_000;
        data.lap_history_data[0].lap_valid_bit_flags = 3;
    }
    apply(&mut model, &partial);
    let history = model.car(23).unwrap().session_history();
    assert!(history.laps.is_empty());
    assert_eq!(history.best_lap_number, None);
    assert_eq!(history.sectors.len(), 1);
    assert_eq!(history.best_sectors_ms, [Some(30_000), None, None]);
}

#[test]
fn assumed_authoritative_completion_cannot_be_overwritten_by_a_later_lap_transition() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(89.0, 1, 0, 0));
    let mut history = packet(2026, 11, 90.0);
    if let Packet::SessionHistory(data) = &mut history.packet {
        data.car_idx = 23;
        data.num_laps = 1;
        data.lap_history_data[0].lap_time_in_ms = 90_000;
        data.lap_history_data[0].sector3_time_ms_part = 30_000;
        data.lap_history_data[0].lap_valid_bit_flags = 0; // Invalid according to the authority.
    }
    apply(&mut model, &history);
    apply(&mut model, &lap(90.1, 2, 90_000, 0));
    let history = model.car(23).unwrap().session_history();
    assert!(history.laps[0].authoritative);
    assert!(!history.laps[0].valid);
    assert_eq!(history.best_lap_number, None);
}

#[test]
fn assumed_both_formats_retain_full_session_laps_beyond_the_history_packet_capacity() {
    for format in [2025, 2026] {
        let mut model = SessionModel::default();
        let mut input = packet(format, 2, 0.0);
        for number in 1..=111 {
            input.header.session_time = f32::from(number - 1) * 90.0 + 0.1;
            if let Packet::Lap(data) = &mut input.packet {
                for value in data.lap_data.iter_mut().flatten() {
                    value.result_status = 2;
                    value.current_lap_num = number;
                    value.current_lap_time_in_ms = 100;
                    value.last_lap_time_in_ms = 90_000;
                }
            }
            apply(&mut model, &input);
        }
        let last_car = if format == 2025 { 21 } else { 23 };
        let history = model.car(last_car).unwrap().session_history();
        assert_eq!(history.laps.len(), 110);
        assert_eq!(history.laps[0].lap_number, 1);
        assert_eq!(history.laps[109].lap_number, 110);
        assert_eq!(history.best_lap_number, Some(1));
        let checkpoint = model.clone();
        let mut correction = packet(format, 11, 10_000.0);
        if let Packet::SessionHistory(data) = &mut correction.packet {
            data.car_idx = last_car as u8;
            data.num_laps = 100;
            for value in &mut data.lap_history_data {
                value.lap_time_in_ms = 90_000;
                value.lap_valid_bit_flags = 15;
            }
        }
        apply(&mut model, &correction);
        let history = model.car(last_car).unwrap().session_history();
        assert_eq!(history.laps.len(), 110);
        assert!(Arc::ptr_eq(
            &history.laps[109],
            &checkpoint.car(last_car).unwrap().session_history().laps[109]
        ));
    }
}

#[test]
fn assumed_rewind_into_a_pit_visit_reopens_it_and_retains_observed_stop_timing() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    let mut input = lap(10.0, 1, 0, 0);
    if let Packet::Lap(data) = &mut input.packet {
        data.lap_data[23].as_mut().unwrap().pit_status = 2;
    }
    apply(&mut model, &input);
    input.header.session_time = 20.0;
    if let Packet::Lap(data) = &mut input.packet {
        let value = data.lap_data[23].as_mut().unwrap();
        value.pit_lane_time_in_lane_in_ms = 10_000;
        value.pit_stop_timer_in_ms = 2500;
    }
    apply(&mut model, &input);
    apply(&mut model, &lap(30.0, 1, 0, 0)); // Exit packet has already reset its timers.
    let stop = &model.car(23).unwrap().session_history().pit_stops[0];
    assert_eq!(stop.lane_time_ms, Some(10_000));
    assert_eq!(stop.stopped_time_ms, Some(2500));
    model.apply(
        Duration::from_secs(31),
        &event(
            30.0,
            EventDetails::Flashback {
                frame_identifier: 15,
                session_time: 15.0,
            },
        ),
    );
    input.header.session_time = 16.0;
    model.apply(Duration::from_secs(32), &input);
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.pit_stops.len(), 1);
    assert_eq!(history.pit_stops[0].exit_time, None);
    assert_eq!(
        model
            .race_control()
            .iter()
            .filter(|entry| !entry.rewound && matches!(entry.kind, RaceControlKind::PitEntry))
            .count(),
        1
    );
}

#[test]
fn assumed_stint_condition_keeps_observed_wear_without_seeding_new_tyres_or_leaking_restricted_data()
 {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.0, 1, 0, 0));
    let mut participants = packet(2026, 4, 0.0);
    if let Packet::Participants(data) = &mut participants.packet {
        data.num_active_cars = 24;
        data.participants[23].as_mut().unwrap().your_telemetry = 1;
    }
    apply(&mut model, &participants);
    apply(&mut model, &status(1.0, 16, 2));
    let mut damage = packet(2026, 10, 2.0);
    if let Packet::CarDamage(data) = &mut damage.packet {
        data.car_damage_data[23].as_mut().unwrap().tyres_wear = [20.0; 4];
    }
    apply(&mut model, &damage);
    apply(&mut model, &status(3.0, 17, 0));
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.stints[0].tyre_condition, Readout::Live([80.0; 4]));
    assert_eq!(history.stints[1].tyre_condition, Readout::Absent);
    if let Packet::Participants(data) = &mut participants.packet {
        data.participants[23].as_mut().unwrap().your_telemetry = 0;
    }
    participants.header.session_time = 4.0;
    apply(&mut model, &participants);
    damage.header.session_time = 5.0;
    apply(&mut model, &damage);
    assert!(
        model
            .car(23)
            .unwrap()
            .session_history()
            .stints
            .iter()
            .all(|stint| stint.tyre_condition == Readout::Restricted)
    );
}

#[test]
fn assumed_tyres_define_stints_pit_visits_stay_separate_and_history_overrides() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(0.1, 1, 0, 0));
    apply(&mut model, &status(1.0, 16, 3));
    let mut pit = lap(30.0, 1, 0, 0);
    if let Packet::Lap(data) = &mut pit.packet {
        data.lap_data[23].as_mut().unwrap().pit_status = 2;
    }
    apply(&mut model, &pit);
    apply(&mut model, &status(31.0, 16, 3));
    apply(&mut model, &lap(40.0, 1, 0, 0)); // Wing-only/drive-through: no tyre change.
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.stints.len(), 1);
    assert_eq!(history.stints[0].starting_age, Some(3));
    assert_eq!(history.stints[0].used_set(), Some(true));
    assert_eq!(history.pit_stops.len(), 1);
    assert_eq!(history.pit_stops[0].session_time, 30.0);
    assert_eq!(history.pit_stops[0].exit_time, Some(40.0));
    apply(&mut model, &lap(90.1, 2, 90_000, 0));
    apply(&mut model, &status(91.0, 16, 0)); // Same compound, new set.
    apply(&mut model, &lap(180.1, 3, 90_000, 0));
    apply(&mut model, &event(181.0, EventDetails::RedFlag));
    apply(&mut model, &status(182.0, 17, 0)); // Red-flag change, no pit visit.
    assert_eq!(model.car(23).unwrap().session_history().stints.len(), 3);
    let checkpoint = model.clone();
    let mut correction = packet(2026, 11, 183.0);
    if let Packet::SessionHistory(data) = &mut correction.packet {
        data.car_idx = 23;
        data.num_tyre_stints = 2;
        data.tyre_stints_history_data[0].end_lap = 2;
        data.tyre_stints_history_data[0].tyre_actual_compound = 16;
        data.tyre_stints_history_data[0].tyre_visual_compound = 16;
        data.tyre_stints_history_data[1].end_lap = 255;
        data.tyre_stints_history_data[1].tyre_actual_compound = 17;
        data.tyre_stints_history_data[1].tyre_visual_compound = 17;
    }
    apply(&mut model, &correction);
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.stints.len(), 2);
    assert_eq!(history.stints[0].end_lap, Some(2));
    assert_eq!(history.stints[1].start_lap, 3);
    assert_eq!(history.stints[1].end_lap, None);
    assert!(history.stints.iter().all(|stint| stint.authoritative));
    assert_eq!(history.pit_stops.len(), 1);
    assert_eq!(
        checkpoint.car(23).unwrap().session_history().stints.len(),
        3
    );
}

#[test]
fn assumed_all_cars_keep_completed_laps_sectors_and_shared_checkpoint_history() {
    let mut model = SessionModel::default();
    let mut before_line = lap(89.0, 1, 0, 0);
    if let Packet::Lap(data) = &mut before_line.packet {
        for value in data.lap_data.iter_mut().flatten() {
            value.current_lap_time_in_ms = 89_000;
            value.sector = 2;
            value.sector1_time_ms_part = 30_000;
            value.sector2_time_ms_part = 29_000;
        }
    }
    apply(&mut model, &before_line);
    apply(&mut model, &lap(90.1, 2, 90_000, 0));
    let checkpoint = model.clone();
    for index in 0..24 {
        let history = model.car(index).unwrap().session_history();
        assert_eq!(history.laps.len(), 1);
        let completed = &history.laps[0];
        assert_eq!(completed.lap_number, 1);
        assert_eq!(completed.lap_time_ms, 90_000);
        assert_eq!(
            completed.sectors_ms,
            [Some(30_000), Some(29_000), Some(31_000)]
        );
        assert!(completed.valid);
        assert_eq!(history.best_lap_number, Some(1));
        assert_eq!(
            history.best_sectors_ms,
            [Some(30_000), Some(29_000), Some(31_000)]
        );
        assert!(Arc::ptr_eq(
            completed,
            &checkpoint.car(index).unwrap().session_history().laps[0]
        ));
    }
    apply(&mut model, &lap(179.0, 2, 90_000, 1));
    apply(&mut model, &lap(180.1, 3, 89_000, 0));
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.laps.len(), 2);
    assert!(!history.laps[1].valid);
    assert_eq!(history.best_lap_number, Some(1));
    assert_eq!(checkpoint.car(23).unwrap().session_history().laps.len(), 1);
    assert!(Arc::ptr_eq(
        &history.laps[0],
        &checkpoint.car(23).unwrap().session_history().laps[0]
    ));
}

#[test]
fn assumed_session_history_corrects_validity_and_bests_without_adding_partial_laps() {
    let mut model = SessionModel::default();
    apply(&mut model, &lap(89.0, 1, 0, 0));
    apply(&mut model, &lap(90.1, 2, 90_000, 1));
    apply(&mut model, &lap(179.1, 3, 89_000, 0));
    let before = model.clone();
    let mut correction = packet(2026, 11, 180.0);
    if let Packet::SessionHistory(data) = &mut correction.packet {
        data.car_idx = 23;
        data.num_laps = 3; // Includes the incomplete current lap.
        for (index, time) in [90_000, 89_000].into_iter().enumerate() {
            let value = &mut data.lap_history_data[index];
            value.lap_time_in_ms = time;
            value.sector1_time_ms_part = 30_000;
            value.sector2_time_ms_part = 30_000;
            value.sector3_time_ms_part = if index == 0 { 30_000 } else { 29_000 };
            value.lap_valid_bit_flags = if index == 0 { 14 } else { 15 };
        }
    }
    apply(&mut model, &correction);
    let history = model.car(23).unwrap().session_history();
    assert_eq!(history.laps.len(), 2);
    assert!(!history.laps[0].valid);
    assert!(history.laps[1].authoritative);
    assert_eq!(history.best_lap_number, Some(2));
    assert_eq!(
        history.best_sectors_ms,
        [Some(30_000), Some(30_000), Some(29_000)]
    );
    assert_eq!(
        history.laps[0].session_time,
        before.car(23).unwrap().session_history().laps[0].session_time
    );
    assert_eq!(
        before.car(23).unwrap().session_history().best_lap_number,
        Some(1)
    );
    assert_eq!(
        model.car(0).unwrap().session_history().best_lap_number,
        Some(1)
    );
    apply(&mut model, &event(181.0, EventDetails::SessionEnded));
    // The late results burst remains authoritative while ended.
    correction.header.session_time = 182.0;
    if let Packet::SessionHistory(data) = &mut correction.packet {
        data.lap_history_data[1].lap_valid_bit_flags = 0;
    }
    apply(&mut model, &correction);
    assert_eq!(
        model.car(23).unwrap().session_history().best_lap_number,
        None
    );
}
