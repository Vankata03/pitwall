//! Assumed synthetic packet-table tests. No real capture has been validated.
use pitwall_core::telemetry::{Packet, parse_datagram};
use std::process::Command;

fn assumed(format: u16, id: u8, fields: &[(&str, &str)]) -> Vec<u8> {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ));
    command.args([format.to_string(), id.to_string()]);
    for (path, value) in fields {
        command.args(["--set", &format!("{path}={value}")]);
    }
    let output = command
        .output()
        .expect("Python is required for assumed datagram fixtures");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn assumed_motion_normalizes_both_formats_and_preserves_header() {
    for (format, raw_g) in [(2025, "-1.9"), (2026, "-1900")] {
        let bytes = assumed(
            format,
            0,
            &[
                ("car_motion_data.21.g_force_lateral", raw_g),
                ("car_motion_data.21.world_position_y", "68.25"),
                ("header.session_uid", "72623859790382856"),
                ("header.overall_frame_identifier", "9001"),
            ],
        );
        let parsed = parse_datagram(&bytes).unwrap();
        assert_eq!(parsed.header.session_uid, 0x0102030405060708);
        assert_eq!(parsed.header.overall_frame_identifier, 9001);
        let Packet::Motion(data) = parsed.packet else {
            panic!("expected motion")
        };
        let car = data.car_motion_data[21].as_ref().unwrap();
        assert!((car.g_force_lateral + 1.9).abs() < 0.00001);
        assert_eq!(car.world_position_y, 68.25);
        assert_eq!(data.car_motion_data[23].is_some(), format == 2026);
    }
}

#[test]
fn assumed_participants_keep_wide_ids_names_colours_and_restriction_flags() {
    for (format, driver, network, team) in
        [(2025, "255", "201", "104"), (2026, "65535", "1025", "486")]
    {
        let bytes = assumed(
            format,
            4,
            &[
                ("num_active_cars", "22"),
                ("participants.21.driver_id", driver),
                ("participants.21.network_id", network),
                ("participants.21.team_id", team),
                ("participants.21.my_team", "1"),
                ("participants.21.name.0", "65"),
                ("participants.21.name.1", "195"),
                ("participants.21.name.2", "169"),
                ("participants.21.num_colours", "4"),
                ("participants.21.livery_colour.3.red", "234"),
                ("participants.21.show_online_names", "1"),
            ],
        );
        let Packet::Participants(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("participants")
        };
        assert_eq!(data.num_active_cars, 22);
        let car = data.participants[21].as_ref().unwrap();
        assert_eq!(
            (car.driver_id, car.network_id, car.team_id),
            if format == 2025 {
                (255u16, 201u16, 104u16)
            } else {
                (65535u16, 1025u16, 486u16)
            }
        );
        assert_eq!(&car.name[..4], &[65, 195, 169, 0]);
        assert_eq!(car.my_team, 1);
        assert_eq!(car.your_telemetry, 0);
        assert_eq!(car.show_online_names, 1);
        assert_eq!(car.livery_colour[3].red, 234);
    }
}

#[test]
fn assumed_telemetry_normalizes_temperature_and_retains_wheel_order() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            6,
            &[
                ("car_telemetry_data.21.speed", "335"),
                ("car_telemetry_data.21.gear", "-1"),
                ("car_telemetry_data.21.engine_temperature", "123"),
                ("car_telemetry_data.21.tyres_pressure.0", "21.5"),
                ("car_telemetry_data.21.tyres_pressure.3", "23.75"),
                ("suggested_gear", "8"),
            ],
        );
        let Packet::CarTelemetry(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("telemetry")
        };
        let car = data.car_telemetry_data[21].as_ref().unwrap();
        assert_eq!(
            (car.speed, car.gear, car.engine_temperature),
            (335, -1, 123u16)
        );
        assert_eq!(car.tyres_pressure, [21.5, 0.0, 0.0, 23.75]);
        assert_eq!(data.suggested_gear, 8);
    }
}

#[test]
fn assumed_status_normalizes_harvest_limit_without_shifting_deployed_energy() {
    for format in [2025, 2026] {
        let mut fields = vec![
            ("car_status_data.21.ers_deployed_this_lap", "1250000"),
            ("car_status_data.21.fuel_remaining_laps", "-0.75"),
            ("car_status_data.21.network_paused", "1"),
        ];
        if format == 2026 {
            fields.push(("car_status_data.21.ers_harvest_limit_per_lap", "8500000"));
        }
        let Packet::CarStatus(data) = parse_datagram(&assumed(format, 7, &fields)).unwrap().packet
        else {
            panic!("status")
        };
        let car = data.car_status_data[21].as_ref().unwrap();
        assert_eq!(car.ers_deployed_this_lap, 1250000.0);
        assert_eq!(car.fuel_remaining_laps, -0.75);
        assert_eq!(car.network_paused, 1);
        assert_eq!(
            car.ers_harvest_limit_per_lap,
            if format == 2026 {
                Some(8500000.0)
            } else {
                None
            }
        );
    }
}

#[test]
fn assumed_telemetry2_uses_per_car_regulations_not_format() {
    let bytes = assumed(
        2026,
        16,
        &[
            ("car_telemetry2_data.23.regulations_2026", "1"),
            (
                "car_telemetry2_data.23.active_aero_activation_distance",
                "450",
            ),
            ("car_telemetry2_data.23.overtake_active", "1"),
            ("car_telemetry2_data.23.overtake_activation_distance", "700"),
            ("car_telemetry2_data.23.driving_wrong_way", "1"),
        ],
    );
    let Packet::CarTelemetry2(data) = parse_datagram(&bytes).unwrap().packet else {
        panic!("telemetry 2")
    };
    assert_eq!(
        data.car_telemetry2_data[0]
            .as_ref()
            .unwrap()
            .regulations_2026,
        0
    );
    let car = data.car_telemetry2_data[23].as_ref().unwrap();
    assert_eq!(car.regulations_2026, 1);
    assert_eq!(car.active_aero_activation_distance, 450);
    assert_eq!(car.overtake_activation_distance, 700);
    assert_eq!((car.overtake_active, car.driving_wrong_way), (1, 1));
}

#[test]
fn assumed_session_keeps_weather_track_pit_window_and_optional_aero_zones() {
    for format in [2025, 2026] {
        let mut fields = vec![
            ("track_temperature", "-5"),
            ("track_id", "-1"),
            ("track_length", "7004"),
            ("num_weather_forecast_samples", "64"),
            ("weather_forecast_samples.63.rain_percentage", "85"),
            ("pit_stop_window_latest_lap", "23"),
            ("sector3_lap_distance_start", "4500.5"),
        ];
        if format == 2026 {
            fields.extend([
                ("num_active_aero_zones_full", "8"),
                ("active_aero_zones_full.7.zone_start", "0.25"),
                ("drs_zones.3.zone_end", "0.9"),
                ("recurring_rewind_prompt", "1"),
            ]);
        }
        let Packet::Session(data) = parse_datagram(&assumed(format, 1, &fields)).unwrap().packet
        else {
            panic!("session")
        };
        assert_eq!(
            (data.track_temperature, data.track_id, data.track_length),
            (-5, -1, 7004)
        );
        assert_eq!(data.weather_forecast_samples[63].rain_percentage, 85);
        assert_eq!(data.pit_stop_window_latest_lap, 23);
        assert_eq!(data.sector3_lap_distance_start, 4500.5);
        assert_eq!(
            data.num_active_aero_zones_full,
            if format == 2026 { Some(8) } else { None }
        );
        assert_eq!(
            data.active_aero_zones_full
                .as_ref()
                .map(|z| z[7].zone_start),
            if format == 2026 { Some(0.25) } else { None }
        );
    }
}

#[test]
fn assumed_lap_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            2,
            &[
                ("lap_data.21.last_lap_time_in_ms", "114618"),
                ("lap_data.21.lap_distance", "-12.5"),
                ("lap_data.21.result_status", "7"),
                ("lap_data.21.speed_trap_fastest_lap", "255"),
                ("time_trial_rival_car_idx", "255"),
            ],
        );
        let Packet::Lap(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("lap")
        };
        let car = data.lap_data[21].as_ref().unwrap();
        assert_eq!(car.last_lap_time_in_ms, 114618);
        assert_eq!(car.lap_distance, -12.5);
        assert_eq!(
            (
                car.result_status,
                car.speed_trap_fastest_lap,
                data.time_trial_rival_car_idx
            ),
            (7, 255, 255)
        );
    }
}

#[test]
fn assumed_setup_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            5,
            &[
                ("car_setup_data.21.front_camber", "-3.5"),
                ("car_setup_data.21.fuel_load", "75.25"),
                ("next_front_wing_value", "42.5"),
            ],
        );
        let Packet::CarSetup(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("setup")
        };
        let car = data.car_setup_data[21].as_ref().unwrap();
        assert_eq!(
            (car.front_camber, car.fuel_load, data.next_front_wing_value),
            (-3.5, 75.25, 42.5)
        );
    }
}

#[test]
fn assumed_classification_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            8,
            &[
                ("num_cars", "22"),
                ("classification_data.21.total_race_time", "3600.123456789"),
                ("classification_data.21.result_reason", "8"),
                ("classification_data.21.num_tyre_stints", "8"),
                ("classification_data.21.tyre_stints_end_laps.7", "57"),
            ],
        );
        let Packet::FinalClassification(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("classification")
        };
        let car = data.classification_data[21].as_ref().unwrap();
        assert_eq!(data.num_cars, 22);
        assert_eq!(car.total_race_time, 3600.123456789);
        assert_eq!(
            (
                car.result_reason,
                car.num_tyre_stints,
                car.tyre_stints_end_laps[7]
            ),
            (8, 8, 57)
        );
    }
}

#[test]
fn assumed_lobby_decodes_both_formats() {
    for format in [2025, 2026] {
        let team = if format == 2025 { "104" } else { "486" };
        let bytes = assumed(
            format,
            9,
            &[
                ("num_players", "22"),
                ("lobby_players.21.team_id", team),
                ("lobby_players.21.name.0", "90"),
                ("lobby_players.21.ready_status", "2"),
            ],
        );
        let Packet::LobbyInfo(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("lobby")
        };
        let car = data.lobby_players[21].as_ref().unwrap();
        assert_eq!(car.team_id, if format == 2025 { 104u16 } else { 486u16 });
        assert_eq!(
            (data.num_players, car.name[0], car.ready_status),
            (22, 90, 2)
        );
    }
}

#[test]
fn assumed_damage_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            10,
            &[
                ("car_damage_data.21.tyres_wear.0", "12.5"),
                ("car_damage_data.21.tyre_blisters.3", "37"),
                ("car_damage_data.21.engine_seized", "1"),
            ],
        );
        let Packet::CarDamage(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("damage")
        };
        let car = data.car_damage_data[21].as_ref().unwrap();
        assert_eq!(car.tyres_wear, [12.5, 0.0, 0.0, 0.0]);
        assert_eq!((car.tyre_blisters[3], car.engine_seized), (37, 1));
    }
}

#[test]
fn assumed_history_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            11,
            &[
                ("car_idx", "21"),
                ("num_laps", "100"),
                ("num_tyre_stints", "8"),
                ("lap_history_data.99.lap_time_in_ms", "130185"),
                ("lap_history_data.99.sector3_time_minutes_part", "1"),
                ("lap_history_data.99.lap_valid_bit_flags", "15"),
                ("tyre_stints_history_data.7.end_lap", "255"),
            ],
        );
        let Packet::SessionHistory(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("history")
        };
        assert_eq!(
            (data.car_idx, data.num_laps, data.num_tyre_stints),
            (21, 100, 8)
        );
        assert_eq!(data.lap_history_data[99].lap_time_in_ms, 130185);
        assert_eq!(data.lap_history_data[99].sector3_time_minutes_part, 1);
        assert_eq!(data.lap_history_data[99].lap_valid_bit_flags, 15);
        assert_eq!(data.tyre_stints_history_data[7].end_lap, 255);
    }
}

#[test]
fn assumed_tyre_sets_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            12,
            &[
                ("car_idx", "21"),
                ("fitted_idx", "255"),
                ("tyre_set_data.19.lap_delta_time", "-1250"),
                ("tyre_set_data.19.fitted", "1"),
            ],
        );
        let Packet::TyreSets(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("tyre sets")
        };
        assert_eq!((data.car_idx, data.fitted_idx), (21, 255));
        assert_eq!(
            (
                data.tyre_set_data[19].lap_delta_time,
                data.tyre_set_data[19].fitted
            ),
            (-1250, 1)
        );
    }
}

#[test]
fn assumed_motion_ex_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            13,
            &[
                ("suspension_position.0", "-0.125"),
                ("chassis_pitch", "0.25"),
                ("wheel_camber_gain.3", "-0.03125"),
            ],
        );
        let Packet::MotionEx(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("motion ex")
        };
        assert_eq!(data.suspension_position, [-0.125, 0.0, 0.0, 0.0]);
        assert_eq!(data.chassis_pitch, 0.25);
        assert_eq!(data.wheel_camber_gain[3], -0.03125);
    }
}

#[test]
fn assumed_time_trial_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            14,
            &[
                (
                    "player_session_best_data_set.team_id",
                    if format == 2025 { "104" } else { "486" },
                ),
                ("personal_best_data_set.lap_time_in_ms", "114618"),
                ("rival_data_set.valid", "1"),
            ],
        );
        let Packet::TimeTrial(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("time trial")
        };
        assert_eq!(
            data.player_session_best_data_set.team_id,
            if format == 2025 { 104u16 } else { 486u16 }
        );
        assert_eq!(data.personal_best_data_set.lap_time_in_ms, 114618);
        assert_eq!(data.rival_data_set.valid, 1);
    }
}

#[test]
fn assumed_lap_positions_decodes_both_formats() {
    for format in [2025, 2026] {
        let bytes = assumed(
            format,
            15,
            &[
                ("num_laps", "50"),
                ("lap_start", "50"),
                ("position_for_vehicle_idx.49.21", "2"),
            ],
        );
        let Packet::LapPositions(data) = parse_datagram(&bytes).unwrap().packet else {
            panic!("lap positions")
        };
        assert_eq!((data.num_laps, data.lap_start), (50, 50));
        assert_eq!(data.position_for_vehicle_idx[49][21], Some(2));
        assert_eq!(
            data.position_for_vehicle_idx[49][23],
            if format == 2025 { None } else { Some(0) }
        );
    }
}

fn assumed_event(format: u16, code: &[u8; 4], details: &[u8]) -> Vec<u8> {
    let mut bytes = assumed(format, 3, &[]);
    bytes[29..33].copy_from_slice(code);
    bytes[33..33 + details.len()].copy_from_slice(details);
    bytes
}

#[test]
fn assumed_event_decodes_flashback_and_format_specific_collision_severity() {
    use pitwall_core::telemetry::EventDetails;
    for format in [2025, 2026] {
        let bytes = assumed_event(format, b"FLBK", &[0x78, 0x56, 0x34, 0x12, 0, 0, 0x48, 0x41]);
        let Packet::Event(event) = parse_datagram(&bytes).unwrap().packet else {
            panic!("event")
        };
        assert_eq!(
            event.details,
            EventDetails::Flashback {
                frame_identifier: 0x12345678,
                session_time: 12.5
            }
        );
        let bytes = assumed_event(format, b"COLL", &[21, 5, 2]);
        let Packet::Event(event) = parse_datagram(&bytes).unwrap().packet else {
            panic!("event")
        };
        assert_eq!(
            event.details,
            EventDetails::Collision {
                vehicle1_idx: 21,
                vehicle2_idx: 5,
                severity: if format == 2026 { Some(2) } else { None }
            }
        );
    }
}

#[test]
fn assumed_all_packet_sizes_match_documented_literals_and_reject_partial_or_extra_bytes() {
    use pitwall_core::telemetry::ParseError;
    let sizes_2025 = [
        1349, 753, 1285, 45, 1284, 1133, 1352, 1239, 1042, 954, 1041, 1460, 231, 273, 101, 1131,
    ];
    let sizes_2026 = [
        1325, 926, 1399, 45, 1470, 1233, 1448, 1445, 1134, 1062, 1133, 1460, 231, 273, 104, 1231,
        269,
    ];
    for (format, sizes) in [(2025, sizes_2025.as_slice()), (2026, sizes_2026.as_slice())] {
        for (id, &size) in sizes.iter().enumerate() {
            let mut bytes = assumed(format, id as u8, &[]);
            assert_eq!(bytes.len(), size, "assumed format {format}, packet {id}");
            assert!(parse_datagram(&bytes).is_ok());
            for length in 0..size {
                assert!(
                    parse_datagram(&bytes[..length]).is_err(),
                    "accepted partial packet {id}, {length}"
                );
            }
            bytes.push(0);
            assert_eq!(
                parse_datagram(&bytes),
                Err(ParseError::InvalidLength {
                    expected: size,
                    actual: size + 1
                })
            );
        }
    }
}

#[test]
fn assumed_unsupported_inputs_are_reported_without_guessing_layouts() {
    use pitwall_core::telemetry::ParseError;
    assert_eq!(
        parse_datagram(&2024u16.to_le_bytes()),
        Err(ParseError::UnsupportedFormat(2024))
    );
    let mut bytes = assumed(2025, 0, &[("header.game_year", "0")]);
    assert!(parse_datagram(&bytes).is_ok());
    bytes[6] = 16;
    assert_eq!(
        parse_datagram(&bytes),
        Err(ParseError::UnsupportedPacketId {
            format: 2025,
            id: 16
        })
    );
    bytes[6] = 255;
    assert_eq!(
        parse_datagram(&bytes),
        Err(ParseError::UnsupportedPacketId {
            format: 2025,
            id: 255
        })
    );
    bytes[6] = 0;
    bytes[5] = 2;
    assert_eq!(
        parse_datagram(&bytes),
        Err(ParseError::UnsupportedPacketVersion { id: 0, version: 2 })
    );
}

#[test]
fn assumed_literal_wire_offsets_decode_independently_of_the_builder_tables() {
    // Literal byte offsets from the research tables, not computed from the schema.
    let mut bytes = vec![0; 1325];
    bytes[..7].copy_from_slice(&[0xea, 0x07, 26, 1, 26, 1, 0]);
    bytes[29 + 23 * 54 + 36..29 + 23 * 54 + 38].copy_from_slice(&(-1900i16).to_le_bytes());
    bytes[29 + 23 * 54 + 42..29 + 23 * 54 + 46].copy_from_slice(&0.5f32.to_le_bytes());
    let Packet::Motion(data) = parse_datagram(&bytes).unwrap().packet else {
        panic!("motion")
    };
    let car = data.car_motion_data[23].as_ref().unwrap();
    assert_eq!((car.g_force_lateral, car.yaw), (-1.9, 0.5));

    let mut bytes = vec![0; 1470];
    bytes[..7].copy_from_slice(&[0xea, 0x07, 26, 1, 26, 1, 4]);
    bytes[29] = 24;
    bytes[30 + 23 * 60 + 1..30 + 23 * 60 + 7].copy_from_slice(&[0xff, 0xff, 1, 4, 0xe6, 1]);
    bytes[30 + 23 * 60 + 59] = 193;
    let Packet::Participants(data) = parse_datagram(&bytes).unwrap().packet else {
        panic!("participants")
    };
    let car = data.participants[23].as_ref().unwrap();
    assert_eq!(
        (car.driver_id, car.network_id, car.team_id),
        (65535, 1025, 486)
    );
    assert_eq!(car.livery_colour[3].blue, 193);

    let mut bytes = vec![0; 1445];
    bytes[..7].copy_from_slice(&[0xea, 0x07, 26, 1, 26, 1, 7]);
    bytes[29 + 23 * 59 + 50..29 + 23 * 59 + 54].copy_from_slice(&8500000f32.to_le_bytes());
    bytes[29 + 23 * 59 + 54..29 + 23 * 59 + 58].copy_from_slice(&1250000f32.to_le_bytes());
    let Packet::CarStatus(data) = parse_datagram(&bytes).unwrap().packet else {
        panic!("status")
    };
    let car = data.car_status_data[23].as_ref().unwrap();
    assert_eq!(
        (car.ers_harvest_limit_per_lap, car.ers_deployed_this_lap),
        (Some(8500000.0), 1250000.0)
    );
}

#[test]
fn assumed_events_preserve_unknown_details_and_decode_every_documented_code() {
    use pitwall_core::telemetry::EventDetails::*;
    for format in [2025, 2026] {
        for (code, expected) in [
            (*b"SSTA", SessionStarted),
            (*b"SEND", SessionEnded),
            (
                *b"FTLP",
                FastestLap {
                    vehicle_idx: 0,
                    lap_time: 0.0,
                },
            ),
            (
                *b"RTMT",
                Retirement {
                    vehicle_idx: 0,
                    reason: 0,
                },
            ),
            (*b"DRSE", DrsEnabled),
            (*b"DRSD", DrsDisabled { reason: 0 }),
            (*b"TMPT", TeamMateInPits { vehicle_idx: 0 }),
            (*b"CHQF", ChequeredFlag),
            (*b"RCWN", RaceWinner { vehicle_idx: 0 }),
            (
                *b"PENA",
                Penalty {
                    penalty_type: 0,
                    infringement_type: 0,
                    vehicle_idx: 0,
                    other_vehicle_idx: 0,
                    time: 0,
                    lap_num: 0,
                    places_gained: 0,
                },
            ),
            (
                *b"SPTP",
                SpeedTrap {
                    vehicle_idx: 0,
                    speed: 0.0,
                    is_overall_fastest_in_session: 0,
                    is_driver_fastest_in_session: 0,
                    fastest_vehicle_idx_in_session: 0,
                    fastest_speed_in_session: 0.0,
                },
            ),
            (*b"STLG", StartLights { num_lights: 0 }),
            (*b"LGOT", LightsOut),
            (*b"DTSV", DriveThroughServed { vehicle_idx: 0 }),
            (
                *b"SGSV",
                StopGoServed {
                    vehicle_idx: 0,
                    stop_time: 0.0,
                },
            ),
            (
                *b"FLBK",
                Flashback {
                    frame_identifier: 0,
                    session_time: 0.0,
                },
            ),
            (*b"BUTN", Buttons { button_status: 0 }),
            (*b"RDFL", RedFlag),
            (
                *b"OVTK",
                Overtake {
                    overtaking_vehicle_idx: 0,
                    being_overtaken_vehicle_idx: 0,
                },
            ),
            (
                *b"SCAR",
                SafetyCar {
                    safety_car_type: 0,
                    event_type: 0,
                },
            ),
        ] {
            let Packet::Event(event) = parse_datagram(&assumed_event(format, &code, &[]))
                .unwrap()
                .packet
            else {
                panic!("event")
            };
            assert_eq!(event.details, expected);
        }
        let bytes = assumed_event(format, b"NEW!", &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let Packet::Event(event) = parse_datagram(&bytes).unwrap().packet else {
            panic!("event")
        };
        assert_eq!(event.details, Unknown);
        assert_eq!(event.code, *b"NEW!");
        assert_eq!(event.raw_details, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        for (code, expected) in [
            (*b"PMEN", PartialAeroEnabled),
            (*b"PMDI", PartialAeroDisabled),
            (*b"OVEN", OvertakeEnabled),
            (*b"OVDI", OvertakeDisabled),
        ] {
            let Packet::Event(event) = parse_datagram(&assumed_event(format, &code, &[]))
                .unwrap()
                .packet
            else {
                panic!("event")
            };
            assert_eq!(
                event.details,
                if format == 2026 { expected } else { Unknown }
            );
        }
    }
}

#[test]
fn assumed_event_union_uses_packed_offsets_for_penalties_speed_and_stop_time() {
    use pitwall_core::telemetry::EventDetails;
    for format in [2025, 2026] {
        for (code, payload, expected) in [
            (
                *b"PENA",
                vec![3, 31, 21, 255, 10, 57, 2],
                EventDetails::Penalty {
                    penalty_type: 3,
                    infringement_type: 31,
                    vehicle_idx: 21,
                    other_vehicle_idx: 255,
                    time: 10,
                    lap_num: 57,
                    places_gained: 2,
                },
            ),
            (
                *b"SPTP",
                vec![21, 0, 128, 167, 67, 1, 0, 5, 0, 0, 170, 67],
                EventDetails::SpeedTrap {
                    vehicle_idx: 21,
                    speed: 335.0,
                    is_overall_fastest_in_session: 1,
                    is_driver_fastest_in_session: 0,
                    fastest_vehicle_idx_in_session: 5,
                    fastest_speed_in_session: 340.0,
                },
            ),
            (
                *b"SGSV",
                vec![21, 0, 0, 40, 65],
                EventDetails::StopGoServed {
                    vehicle_idx: 21,
                    stop_time: 10.5,
                },
            ),
            (
                *b"RTMT",
                vec![21, 8],
                EventDetails::Retirement {
                    vehicle_idx: 21,
                    reason: 8,
                },
            ),
            (*b"DRSD", vec![3], EventDetails::DrsDisabled { reason: 3 }),
        ] {
            let Packet::Event(event) = parse_datagram(&assumed_event(format, &code, &payload))
                .unwrap()
                .packet
            else {
                panic!("event")
            };
            assert_eq!(event.details, expected);
        }
    }
}
