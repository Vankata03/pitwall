use super::lap_timing::*;
use pitwall_core::{
    model::SessionModel,
    readout::ReadoutState,
    telemetry::{Packet, ParsedDatagram, parse_datagram},
};
use serde_json::json;
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
fn running(model: &mut SessionModel, s1: u16, s2: u16) {
    let mut lap = assumed(2);
    let Packet::Lap(data) = &mut lap.packet else {
        unreachable!()
    };
    for car in data.lap_data.iter_mut().flatten() {
        car.result_status = 2;
    }
    let data = data.lap_data[0].as_mut().unwrap();
    data.result_status = 2;
    data.current_lap_num = 2;
    data.current_lap_time_in_ms = 60000;
    data.sector = 2;
    data.sector1_time_ms_part = s1;
    data.sector2_time_ms_part = s2;
    model.apply(Duration::ZERO, &lap);
}
#[test]
fn time_trial_references_show_plain_times_without_seeding_a_live_delta() {
    let mut model = SessionModel::default();
    let mut session = assumed(1);
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.session_type = 18;
    model.apply(Duration::ZERO, &session);
    running(&mut model, 27102, 28500);
    let mut tt = assumed(14);
    let Packet::TimeTrial(data) = &mut tt.packet else {
        unreachable!()
    };
    data.personal_best_data_set.lap_time_in_ms = 83000;
    data.personal_best_data_set.sector1_time_in_ms = 27000;
    data.personal_best_data_set.sector2_time_in_ms = 28000;
    data.personal_best_data_set.sector3_time_in_ms = 28000;
    data.rival_data_set.lap_time_in_ms = 82500;
    data.rival_data_set.sector1_time_in_ms = 26900;
    model.apply(Duration::ZERO, &tt);
    for (key, label, time, delta) in [
        ("personal-best", "Personal best", "1:23.000", "+0.102"),
        ("rival", "Rival", "1:22.500", "+0.202"),
    ] {
        let v = view(&Inputs::of(
            model.car(0),
            LapReference::of(&json!({"reference": key})),
            model.lifecycle(),
            (500.0, 300.0),
        ));
        assert_eq!(v.delta_label, label);
        assert_eq!(v.delta.state, ReadoutState::Absent);
        assert_eq!(v.reference.unwrap().1.text, time);
        assert_eq!(v.sectors.unwrap()[0].delta.text, delta);
        if key == "personal-best" {
            assert_eq!(
                v.reference_sectors.unwrap().map(|s| s.text),
                ["27.000", "28.000", "28.000"]
            );
        }
    }
}
#[test]
fn sectors_rank_against_other_cars_and_prior_laps_with_glyphs() {
    let mut model = SessionModel::default();
    for (car, s1, s2) in [(0, 27300, 28600), (1, 27200, 28400)] {
        let mut history = assumed(11);
        let Packet::SessionHistory(data) = &mut history.packet else {
            unreachable!()
        };
        data.car_idx = car;
        data.num_laps = 1;
        data.lap_history_data[0].lap_time_in_ms = 85000;
        data.lap_history_data[0].sector1_time_ms_part = s1;
        data.lap_history_data[0].sector2_time_ms_part = s2;
        data.lap_history_data[0].lap_valid_bit_flags = 15;
        model.apply(Duration::ZERO, &history);
    }
    running(&mut model, 27102, 28500);
    let v = view(&Inputs::from_model(
        &model,
        Some(0),
        LapReference::Best,
        (500.0, 300.0),
    ));
    let sectors = v.sectors.unwrap();
    assert_eq!(sectors[0].time.text, "27.102 \u{25c6}");
    assert_eq!(sectors[1].time.text, "28.500 \u{25cf}");
}
#[test]
fn missing_time_trial_references_never_fall_back_to_best_or_zero() {
    let mut model = SessionModel::default();
    running(&mut model, 27102, 28500);
    for key in ["personal-best", "rival"] {
        let v = view(&Inputs::from_model(
            &model,
            Some(0),
            LapReference::of(&json!({"reference": key})),
            (500.0, 300.0),
        ));
        assert_eq!(v.delta.state, ReadoutState::Absent);
        assert_eq!(v.reference.unwrap().1.state, ReadoutState::Absent);
        assert!(
            v.reference_sectors
                .unwrap()
                .iter()
                .all(|s| s.state == ReadoutState::Absent)
        );
        assert_eq!(v.sectors.unwrap()[0].delta.state, ReadoutState::Absent);
    }
}
#[test]
fn time_trial_completed_result_expires_and_reference_data_stays_player_only() {
    let mut model = SessionModel::default();
    let mut session = assumed(1);
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.session_type = 18;
    model.apply(Duration::ZERO, &session);
    running(&mut model, 27102, 28500);
    let mut tt = assumed(14);
    let Packet::TimeTrial(data) = &mut tt.packet else {
        unreachable!()
    };
    for reference in [&mut data.personal_best_data_set, &mut data.rival_data_set] {
        reference.lap_time_in_ms = 84000;
        reference.sector1_time_in_ms = 27000;
        reference.sector2_time_in_ms = 0;
        reference.sector3_time_in_ms = 28500;
    }
    model.apply(Duration::ZERO, &tt);
    let mut finish = assumed(2);
    finish.header.session_time = 84.381;
    let Packet::Lap(data) = &mut finish.packet else {
        unreachable!()
    };
    let lap = data.lap_data[0].as_mut().unwrap();
    lap.result_status = 2;
    lap.current_lap_num = 3;
    lap.current_lap_time_in_ms = 4999;
    lap.last_lap_time_in_ms = 84381;
    model.apply(Duration::from_secs(85), &finish);
    for reference in [LapReference::PersonalBest, LapReference::Rival] {
        let v = view(&Inputs::from_model(
            &model,
            Some(0),
            reference,
            (500.0, 300.0),
        ));
        assert_eq!(v.clock.text, "1:24.381");
        assert_eq!(v.delta.text, "+0.381");
        assert_eq!(v.reference_sectors.unwrap()[1].state, ReadoutState::Absent);
        let other = view(&Inputs::from_model(
            &model,
            Some(1),
            reference,
            (500.0, 300.0),
        ));
        assert_eq!(other.reference.unwrap().1.state, ReadoutState::Absent);
    }
    let Packet::Lap(data) = &mut finish.packet else {
        unreachable!()
    };
    data.lap_data[0].as_mut().unwrap().current_lap_time_in_ms = 5000;
    model.apply(Duration::from_secs(85), &finish);
    for reference in [LapReference::PersonalBest, LapReference::Rival] {
        let v = view(&Inputs::from_model(
            &model,
            Some(0),
            reference,
            (500.0, 300.0),
        ));
        assert_eq!(v.clock.text, "0:05.000");
        assert_eq!(v.delta.state, ReadoutState::Absent);
        assert!(
            v.sectors
                .unwrap()
                .iter()
                .all(|s| s.time.state == ReadoutState::Absent)
        );
    }
    let Packet::Session(data) = &mut session.packet else {
        unreachable!()
    };
    data.session_type = 15;
    model.apply(Duration::from_secs(85), &session);
    let v = view(&Inputs::from_model(
        &model,
        Some(0),
        LapReference::PersonalBest,
        (500.0, 300.0),
    ));
    assert_eq!(v.reference.unwrap().1.state, ReadoutState::Absent);
    let cleared = view(&Inputs::from_model(
        &SessionModel::default(),
        Some(0),
        LapReference::PersonalBest,
        (500.0, 300.0),
    ));
    assert_eq!(cleared.clock.state, ReadoutState::Absent);
    assert_eq!(cleared.delta.state, ReadoutState::Absent);
}
