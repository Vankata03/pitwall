//! Assumed synthetic model tests; packet-table fixtures are not capture evidence.
use pitwall_core::{
    model::{Lifecycle, Readout, Regulations, SessionId, SessionModel},
    telemetry::{ParsedDatagram, parse_datagram},
};
use std::{process::Command, time::Duration};

fn packet(format: u16, id: u8, fields: &[(&str, &str)]) -> ParsedDatagram {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ));
    command.args([format.to_string(), id.to_string()]);
    command.args(["--set", "header.session_uid=101"]);
    for (path, value) in fields {
        command.args(["--set", &format!("{path}={value}")]);
    }
    let output = command
        .output()
        .expect("Python is required for assumed fixtures");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_datagram(&output.stdout).unwrap()
}

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

#[test]
fn assumed_lifecycle_uses_arrival_time_preserves_lost_session_and_resets_identity() {
    let mut model = SessionModel::default();
    assert_eq!(model.lifecycle(), Lifecycle::Waiting);
    model.apply(seconds(1), &packet(2026, 0, &[("header.session_uid", "0")]));
    assert_eq!(model.lifecycle(), Lifecycle::Waiting);
    model.apply(seconds(2), &packet(2026, 0, &[]));
    assert_eq!(
        model.identity(),
        Some(SessionId {
            uid: 101,
            format: 2026
        })
    );
    assert_eq!(model.lifecycle(), Lifecycle::Live);
    model.advance(seconds(31));
    assert_eq!(model.lifecycle(), Lifecycle::Live);
    model.advance(seconds(32));
    assert_eq!(model.lifecycle(), Lifecycle::Lost);
    model.apply(seconds(40), &packet(2026, 0, &[]));
    assert_eq!(model.lifecycle(), Lifecycle::Live);
    let mut ended = packet(2026, 3, &[]);
    if let pitwall_core::telemetry::Packet::Event(event) = &mut ended.packet {
        event.details = pitwall_core::telemetry::EventDetails::SessionEnded;
    }
    model.apply(seconds(41), &ended);
    model.advance(seconds(80));
    assert_eq!(model.lifecycle(), Lifecycle::Ended);
    model.apply(seconds(81), &packet(2025, 0, &[]));
    assert_eq!(
        model.identity(),
        Some(SessionId {
            uid: 101,
            format: 2025
        })
    );
    assert_eq!(model.lifecycle(), Lifecycle::Live);
    model.apply(
        seconds(82),
        &packet(2025, 0, &[("header.session_uid", "102")]),
    );
    assert_eq!(model.identity().unwrap().uid, 102);
}

#[test]
fn assumed_car_validity_identity_takeover_and_retirement_keep_vehicle_index() {
    let mut model = SessionModel::default();
    model.apply(
        seconds(0),
        &packet(
            2026,
            4,
            &[
                ("num_active_cars", "24"),
                ("participants.23.driver_id", "1025"),
                ("participants.23.team_id", "65535"),
                ("participants.23.my_team", "0"),
                ("participants.23.your_telemetry", "1"),
            ],
        ),
    );
    assert!(model.car(23).is_none());
    model.apply(
        seconds(0),
        &packet(
            2026,
            2,
            &[
                ("lap_data.23.result_status", "2"),
                ("lap_data.23.car_position", "4"),
                ("lap_data.0.result_status", "1"),
            ],
        ),
    );
    let car = model.car(23).unwrap();
    assert!(!car.is_my_team());
    assert!(
        matches!(car.participant(), Readout::Live(p) if p.driver_id == 1025 && p.team_id == 65535)
    );
    assert!(model.car(0).is_none());
    assert!(model.car(24).is_none());
    model.apply(
        seconds(1),
        &packet(
            2026,
            4,
            &[
                ("num_active_cars", "24"),
                ("participants.23.driver_id", "1025"),
                ("participants.23.ai_controlled", "1"),
                ("participants.23.my_team", "1"),
            ],
        ),
    );
    assert!(model.car(23).unwrap().is_my_team());
    model.apply(
        seconds(1),
        &packet(
            2026,
            3,
            &[
                ("event_string_code.0", "82"),
                ("event_string_code.1", "84"),
                ("event_string_code.2", "77"),
                ("event_string_code.3", "84"),
                ("event_details.0", "23"),
                ("event_details.1", "9"),
            ],
        ),
    );
    model.apply(
        seconds(2),
        &packet(2026, 2, &[("lap_data.23.result_status", "0")]),
    );
    let car = model.car(23).unwrap();
    assert!(car.retired());
    assert_eq!(car.retirement_reason(), Some(9));
    assert!(matches!(car.lap(), Readout::Stale(lap) if lap.car_position == 4));
    model.apply(
        seconds(3),
        &packet(2026, 0, &[("header.session_uid", "102")]),
    );
    assert!(model.car(23).is_none());
}

#[test]
fn assumed_freshness_is_per_group_per_car_and_uses_configured_menu_interval() {
    let mut model = SessionModel::new(Duration::from_millis(50));
    model.apply(
        seconds(0),
        &packet(
            2026,
            2,
            &[
                ("lap_data.0.result_status", "2"),
                ("lap_data.1.result_status", "2"),
            ],
        ),
    );
    model.apply(
        seconds(0),
        &packet(2026, 6, &[("car_telemetry_data.0.speed", "300")]),
    );
    model.apply(
        seconds(0),
        &packet(2026, 0, &[("car_motion_data.0.world_position_x", "42")]),
    );
    model.apply(
        seconds(0),
        &packet(2026, 11, &[("car_idx", "0"), ("num_laps", "3")]),
    );
    model.advance(Duration::from_millis(149));
    assert!(matches!(model.car(0).unwrap().telemetry(), Readout::Live(t) if t.speed == 300));
    model.advance(Duration::from_millis(150));
    assert!(matches!(model.car(0).unwrap().telemetry(), Readout::Stale(t) if t.speed == 300));
    model.apply(
        Duration::from_millis(150),
        &packet(2026, 6, &[("car_telemetry_data.0.speed", "301")]),
    );
    assert!(matches!(model.car(0).unwrap().telemetry(), Readout::Live(t) if t.speed == 301));
    assert!(
        matches!(model.car(0).unwrap().motion(), Readout::Stale(m) if m.world_position_x == 42.0)
    );
    assert!(matches!(model.car(1).unwrap().history(), Readout::Absent));
    model.apply(seconds(2), &packet(2026, 11, &[("car_idx", "1")]));
    model.advance(seconds(3));
    assert!(matches!(model.car(0).unwrap().history(), Readout::Stale(h) if h.num_laps == 3));
    assert!(matches!(model.car(1).unwrap().history(), Readout::Live(_)));
    model.advance(seconds(1));
    assert!(matches!(model.car(0).unwrap().history(), Readout::Stale(_)));
}

#[test]
fn assumed_restricted_values_are_discarded_while_public_status_and_temperatures_survive() {
    let mut model = SessionModel::default();
    model.apply(
        seconds(0),
        &packet(2026, 2, &[("lap_data.0.result_status", "2")]),
    );
    let status = packet(
        2026,
        7,
        &[
            ("car_status_data.0.fuel_in_tank", "42"),
            ("car_status_data.0.ers_store_energy", "2000000"),
            ("car_status_data.0.front_brake_bias", "58"),
            ("car_status_data.0.actual_tyre_compound", "18"),
            ("car_status_data.0.tyres_age_laps", "4"),
            ("car_status_data.0.drs_allowed", "1"),
        ],
    );
    // Unknown permission cannot expose values before Participants arrives.
    model.apply(seconds(0), &status);
    assert!(matches!(model.car(0).unwrap().fuel(), Readout::Absent));
    model.apply(seconds(0), &packet(2026, 4, &[("num_active_cars", "1")]));
    model.apply(seconds(0), &status);
    model.apply(
        seconds(0),
        &packet(2026, 10, &[("car_damage_data.0.tyres_wear.0", "20")]),
    );
    model.apply(seconds(0), &packet(2026, 12, &[("car_idx", "0")]));
    model.apply(
        seconds(0),
        &packet(
            2026,
            6,
            &[
                ("car_telemetry_data.0.engine_temperature", "110"),
                ("car_telemetry_data.0.tyres_pressure.0", "22.5"),
            ],
        ),
    );
    let car = model.car(0).unwrap();
    assert!(matches!(car.fuel(), Readout::Restricted));
    assert!(matches!(car.ers(), Readout::Restricted));
    assert!(matches!(car.front_brake_bias(), Readout::Restricted));
    assert!(matches!(car.damage(), Readout::Restricted));
    assert!(matches!(car.tyre_sets(), Readout::Restricted));
    assert!(
        matches!(car.status(), Readout::Live(s) if s.actual_tyre_compound == 18 && s.tyres_age_laps == 4 && s.drs_allowed == 1)
    );
    assert!(
        matches!(car.telemetry(), Readout::Live(t) if t.engine_temperature == 110 && t.tyres_pressure[0] == 22.5)
    );
    model.advance(seconds(31));
    assert_eq!(model.lifecycle(), Lifecycle::Lost);
    assert!(matches!(model.car(0).unwrap().fuel(), Readout::Restricted));
    assert!(matches!(
        model.car(0).unwrap().telemetry(),
        Readout::Stale(_)
    ));
    // Releasing permission must not resurrect withheld or older public values.
    model.apply(
        seconds(32),
        &packet(
            2026,
            4,
            &[
                ("num_active_cars", "1"),
                ("participants.0.your_telemetry", "1"),
            ],
        ),
    );
    assert!(matches!(model.car(0).unwrap().fuel(), Readout::Absent));
    model.apply(seconds(32), &status);
    assert!(matches!(model.car(0).unwrap().fuel(), Readout::Live(f) if f.in_tank == 42.0));
    assert!(matches!(model.car(0).unwrap().ers(), Readout::Live(e) if e.store_energy == 2000000.0));
    model.apply(
        seconds(32),
        &packet(2026, 10, &[("car_damage_data.0.tyres_wear.0", "20")]),
    );
    model.apply(
        seconds(32),
        &packet(2026, 12, &[("car_idx", "0"), ("fitted_idx", "3")]),
    );
    assert!(matches!(model.car(0).unwrap().damage(), Readout::Live(d) if d.tyres_wear[0] == 20.0));
    assert!(matches!(model.car(0).unwrap().tyre_sets(), Readout::Live(t) if t.fitted_idx == 3));
    model.advance(Duration::from_millis(32300));
    assert!(matches!(model.car(0).unwrap().damage(), Readout::Stale(_)));
    assert!(matches!(
        model.car(0).unwrap().tyre_sets(),
        Readout::Live(_)
    ));
    model.apply(seconds(33), &packet(2026, 4, &[("num_active_cars", "1")]));
    assert!(matches!(model.car(0).unwrap().fuel(), Readout::Restricted));
}

#[test]
fn assumed_latest_forecast_and_pit_window_follow_player_and_spectator_without_estimates() {
    let mut model = SessionModel::default();
    assert_eq!(model.player_car_index(), None);
    assert!(matches!(model.forecast(), Readout::Absent));
    model.apply(
        seconds(0),
        &packet(
            2026,
            1,
            &[
                ("header.player_car_index", "1"),
                ("num_weather_forecast_samples", "2"),
                ("weather_forecast_samples.0.weather", "2"),
                ("weather_forecast_samples.1.rain_percentage", "80"),
                ("pit_stop_window_ideal_lap", "12"),
                ("pit_stop_window_latest_lap", "15"),
                ("pit_stop_rejoin_position", "7"),
            ],
        ),
    );
    assert_eq!(model.player_car_index(), Some(1));
    assert!(
        matches!(model.forecast(), Readout::Live(f) if f.len() == 2 && f[1].rain_percentage == 80)
    );
    assert!(
        matches!(model.pit_window(1), Readout::Live(w) if w.ideal_lap == 12 && w.latest_lap == 15 && w.rejoin_position == 7)
    );
    assert!(matches!(model.pit_window(0), Readout::Absent));
    model.advance(Duration::from_millis(1500));
    assert!(matches!(model.forecast(), Readout::Stale(_)));
    model.apply(
        seconds(2),
        &packet(
            2026,
            1,
            &[
                ("header.player_car_index", "1"),
                ("is_spectating", "1"),
                ("spectator_car_index", "23"),
                ("num_weather_forecast_samples", "1"),
                ("weather_forecast_samples.0.weather", "4"),
                ("pit_stop_window_ideal_lap", "20"),
                ("pit_stop_window_latest_lap", "22"),
            ],
        ),
    );
    model.apply(
        seconds(2),
        &packet(2026, 0, &[("header.player_car_index", "0")]),
    );
    assert_eq!(model.player_car_index(), Some(23));
    assert!(matches!(model.forecast(), Readout::Live(f) if f.len() == 1 && f[0].weather == 4));
    assert!(matches!(model.pit_window(23), Readout::Live(w) if w.ideal_lap == 20));
    assert!(matches!(model.pit_window(1), Readout::Absent));
    model.apply(
        seconds(3),
        &packet(
            2026,
            1,
            &[
                ("is_spectating", "1"),
                ("spectator_car_index", "255"),
                ("num_weather_forecast_samples", "255"),
            ],
        ),
    );
    assert_eq!(model.player_car_index(), None);
    assert!(matches!(model.forecast(), Readout::Live(f) if f.len() == 64));
    assert!(matches!(model.pit_window(255), Readout::Absent));
}

#[test]
fn assumed_ended_session_accepts_only_late_history_and_classification_with_time_as_sent() {
    let mut model = SessionModel::default();
    model.apply(
        seconds(0),
        &packet(2026, 2, &[("lap_data.0.result_status", "2")]),
    );
    model.apply(
        seconds(0),
        &packet(2026, 6, &[("car_telemetry_data.0.speed", "100")]),
    );
    let mut ended = packet(2026, 3, &[]);
    if let pitwall_core::telemetry::Packet::Event(event) = &mut ended.packet {
        event.details = pitwall_core::telemetry::EventDetails::SessionEnded;
    }
    model.apply(seconds(1), &ended);
    model.apply(
        seconds(2),
        &packet(2026, 6, &[("car_telemetry_data.0.speed", "250")]),
    );
    model.apply(
        seconds(23),
        &packet(
            2026,
            11,
            &[("num_laps", "9"), ("header.overall_frame_identifier", "0")],
        ),
    );
    assert_eq!(model.lifecycle(), Lifecycle::Ended);
    assert!(matches!(model.car(0).unwrap().telemetry(), Readout::Stale(t) if t.speed == 100));
    assert!(matches!(model.car(0).unwrap().history(), Readout::Live(h) if h.num_laps == 9));
    let final_packet = packet(
        2026,
        8,
        &[
            ("header.session_time", "123.5"),
            ("num_cars", "1"),
            ("classification_data.0.result_status", "3"),
            ("classification_data.0.total_race_time", "120.25"),
        ],
    );
    model.apply(seconds(24), &final_packet);
    let classification = model.final_classification().unwrap();
    assert_eq!(classification.session_time, 123.5);
    assert_eq!(
        classification.data.classification_data[0]
            .as_ref()
            .unwrap()
            .total_race_time,
        120.25
    );
    // Final Classification itself can be the first packet; SSTA is unnecessary.
    let mut standalone = SessionModel::default();
    standalone.apply(seconds(24), &final_packet);
    assert_eq!(standalone.lifecycle(), Lifecycle::Ended);
    assert!(standalone.car(0).is_some());
    model.apply(
        seconds(25),
        &packet(2026, 0, &[("header.session_uid", "102")]),
    );
    assert!(model.final_classification().is_none());
    assert!(model.car(0).is_none());
}

#[test]
fn assumed_both_formats_keep_per_car_regulations_and_checkpoint_clones_independent() {
    for format in [2025, 2026] {
        let mut model = SessionModel::default();
        model.apply(
            seconds(0),
            &packet(format, 2, &[("lap_data.0.result_status", "2")]),
        );
        model.apply(
            seconds(0),
            &packet(format, 6, &[("car_telemetry_data.0.speed", "200")]),
        );
        if format == 2026 {
            assert_eq!(model.car(0).unwrap().regulations(), None);
            model.apply(
                seconds(0),
                &packet(
                    format,
                    16,
                    &[("car_telemetry2_data.0.regulations_2026", "1")],
                ),
            );
            assert_eq!(
                model.car(0).unwrap().regulations(),
                Some(Regulations::Regulations2026)
            );
        } else {
            assert_eq!(
                model.car(0).unwrap().regulations(),
                Some(Regulations::Pre2026)
            );
            assert!(matches!(model.car(0).unwrap().aero(), Readout::Absent));
            assert!(model.car(22).is_none());
        }
        let checkpoint = model.clone();
        // Readouts borrow the model, so retaining one needs no retained CarView.
        let frozen_telemetry = checkpoint.car(0).unwrap().telemetry();
        model.apply(
            seconds(1),
            &packet(format, 6, &[("car_telemetry_data.0.speed", "250")]),
        );
        assert!(matches!(frozen_telemetry, Readout::Live(t) if t.speed == 200));
        assert!(matches!(model.car(0).unwrap().telemetry(), Readout::Live(t) if t.speed == 250));
        let mut replay = checkpoint.clone();
        replay.apply(
            seconds(1),
            &packet(format, 6, &[("car_telemetry_data.0.speed", "250")]),
        );
        assert_eq!(replay, model);
        replay.advance(seconds(31));
        assert_eq!(replay.lifecycle(), Lifecycle::Lost);
        assert_eq!(model.lifecycle(), Lifecycle::Live);
        assert_eq!(checkpoint.lifecycle(), Lifecycle::Live);
        if format == 2026 {
            model.apply(
                seconds(2),
                &packet(
                    format,
                    16,
                    &[("car_telemetry2_data.0.regulations_2026", "0")],
                ),
            );
            assert_eq!(
                model.car(0).unwrap().regulations(),
                Some(Regulations::Pre2026)
            );
            assert_eq!(
                checkpoint.car(0).unwrap().regulations(),
                Some(Regulations::Regulations2026)
            );
        }
    }
}
