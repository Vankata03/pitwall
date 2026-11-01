//! Assumed synthetic data; not verified game telemetry.
use pitwall_collector::{
    model::{Readout, Reference, SessionModel},
    telemetry::{Packet, ParsedDatagram, parse_datagram},
};
use std::{process::Command, time::Duration};
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

fn point(model: &mut SessionModel, number: u8, distance: f32, ms: u32, last: u32, time: f32) {
    let mut p = lap(time, number, last, 0);
    if let Packet::Lap(data) = &mut p.packet {
        for v in data.lap_data.iter_mut().flatten() {
            v.lap_distance = distance;
            v.current_lap_time_in_ms = ms;
        }
    }
    apply(model, &p);
}
#[test]
fn assumed_all_cars_interpolate_named_reference_and_keep_shared_trace() {
    let mut m = SessionModel::default();
    point(&mut m, 1, 0.0, 0, 0, 0.0);
    point(&mut m, 1, 10.0, 1000, 0, 1.0);
    point(&mut m, 2, 0.0, 0, 2000, 2.0);
    let checkpoint = m.clone();
    point(&mut m, 2, 7.5, 700, 2000, 2.7);
    for index in 0..24 {
        let car = m.car(index).unwrap();
        assert_eq!(car.live_delta(Reference::Best), Readout::Live(-50.0));
        assert_eq!(car.live_delta(Reference::Previous), Readout::Live(-50.0));
        let trace = car.lap_trace(Reference::Best).unwrap();
        assert_eq!(trace.samples[1].distance_m, 5.0);
        assert_eq!(trace.samples[1].time_ms, 500.0);
        assert!(std::sync::Arc::ptr_eq(
            trace,
            checkpoint
                .car(index)
                .unwrap()
                .lap_trace(Reference::Best)
                .unwrap()
        ));
    }
}

#[test]
fn assumed_official_line_results_use_prior_references_until_next_sector() {
    let mut m = SessionModel::default();
    point(&mut m, 1, 0.0, 0, 0, 0.0);
    point(&mut m, 2, 0.0, 0, 2000, 2.0);
    point(&mut m, 3, 0.0, 0, 1800, 3.8);
    let result = m
        .car(23)
        .unwrap()
        .completed_result(Reference::Best)
        .unwrap();
    assert_eq!(result.lap_number, 2);
    assert_eq!(result.delta_ms, Some(-200));
    assert_eq!(result.reference_lap_number, Some(1));
    assert_eq!(
        m.car(23)
            .unwrap()
            .reference_lap(Reference::Best)
            .unwrap()
            .lap_number,
        2
    );
    let mut p = lap(4.8, 3, 1800, 0);
    if let Packet::Lap(d) = &mut p.packet {
        d.lap_data[23].as_mut().unwrap().sector1_time_ms_part = 1000;
    }
    apply(&mut m, &p);
    assert!(
        m.car(23)
            .unwrap()
            .completed_result(Reference::Best)
            .is_none()
    );
}

#[test]
fn assumed_gaps_negative_distance_invalid_laps_and_missing_named_samples() {
    let mut m = SessionModel::default();
    point(&mut m, 1, -5.0, 0, 0, 0.0);
    point(&mut m, 1, 0.0, 0, 0, 0.1);
    point(&mut m, 1, 100.0, 1000, 0, 1.1);
    point(&mut m, 1, 205.0, 2000, 0, 2.1);
    point(&mut m, 2, 0.0, 0, 3000, 3.0);
    point(&mut m, 2, 50.0, 400, 3000, 3.5);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Best),
        Readout::Live(-100.0)
    );
    point(&mut m, 2, 150.0, 1400, 3000, 4.5);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Best),
        Readout::Absent
    );
    let p = lap(5.0, 2, 3000, 1);
    apply(&mut m, &p);
    point(&mut m, 3, 0.0, 0, 2500, 5.1);
    assert!(
        !m.car(0)
            .unwrap()
            .reference_lap(Reference::Previous)
            .unwrap()
            .valid
    );
    assert_eq!(
        m.car(0)
            .unwrap()
            .reference_lap(Reference::Best)
            .unwrap()
            .lap_number,
        1
    );
    let mut history = packet(2026, 11, 5.2);
    if let Packet::SessionHistory(d) = &mut history.packet {
        d.car_idx = 0;
        d.num_laps = 2;
        d.lap_history_data[0].lap_time_in_ms = 3000;
        d.lap_history_data[0].lap_valid_bit_flags = 0;
        d.lap_history_data[1].lap_time_in_ms = 2500;
        d.lap_history_data[1].lap_valid_bit_flags = 15;
    }
    apply(&mut m, &history);
    assert_eq!(
        m.car(0)
            .unwrap()
            .reference_lap(Reference::Best)
            .unwrap()
            .lap_number,
        2
    );
    assert!(
        m.car(0)
            .unwrap()
            .completed_result(Reference::Best)
            .unwrap()
            .valid
    );
    let mut h = history.clone();
    if let Packet::SessionHistory(d) = &mut h.packet {
        d.car_idx = 1;
        d.num_laps = 3;
        d.lap_history_data[2].lap_time_in_ms = 2000;
        d.lap_history_data[2].sector3_time_ms_part = 500;
        d.lap_history_data[2].lap_valid_bit_flags = 15;
    }
    apply(&mut m, &h);
    point(&mut m, 3, 5.0, 100, 2500, 5.3);
    assert_eq!(
        m.car(1).unwrap().live_delta(Reference::Best),
        Readout::Absent
    );
}

#[test]
fn assumed_time_trial_extras_are_summary_only_and_sector_deltas_follow_reference() {
    let mut m = SessionModel::default();
    let mut session = packet(2026, 1, 0.0);
    if let Packet::Session(d) = &mut session.packet {
        d.session_type = 18;
    }
    apply(&mut m, &session);
    let mut tt = packet(2026, 14, 0.0);
    if let Packet::TimeTrial(d) = &mut tt.packet {
        d.personal_best_data_set.lap_time_in_ms = 3000;
        d.personal_best_data_set.sector1_time_in_ms = 1000;
    }
    apply(&mut m, &tt);
    point(&mut m, 1, 0.0, 0, 0, 0.0);
    assert!(m.car(0).unwrap().reference_lap(Reference::Best).is_none());
    let mut p = lap(1.1, 1, 0, 0);
    if let Packet::Lap(d) = &mut p.packet {
        d.lap_data[0].as_mut().unwrap().sector1_time_ms_part = 900;
        d.lap_data[0].as_mut().unwrap().sector = 1;
    }
    apply(&mut m, &p);
    assert_eq!(
        m.car(0).unwrap().sector_delta(Reference::PersonalBest, 0),
        Some(-100)
    );
    point(&mut m, 2, 0.0, 0, 2800, 2.8);
    assert_eq!(
        m.car(0)
            .unwrap()
            .completed_result(Reference::PersonalBest)
            .unwrap()
            .delta_ms,
        Some(-200)
    );
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::PersonalBest),
        Readout::Absent
    );
    assert!(
        m.car(0)
            .unwrap()
            .time_trial_reference(Reference::PersonalBest)
            .is_some()
    );
    session.header.session_time = 3.0;
    if let Packet::Session(d) = &mut session.packet {
        d.session_type = 10;
    }
    apply(&mut m, &session);
    assert!(
        m.car(0)
            .unwrap()
            .time_trial_reference(Reference::PersonalBest)
            .is_none()
    );
}

#[test]
fn assumed_flashback_restores_extra_generation_truncates_current_and_bounds_samples() {
    let mut m = SessionModel::default();
    for number in 1..=7 {
        let start = f32::from(number - 1) * 3.0;
        point(&mut m, number, 0.0, 0, 3000, start);
        point(&mut m, number, 10.0, 1000, 3000, start + 1.0);
        point(&mut m, number, 20.0, 2000, 3000, start + 2.0);
    }
    let checkpoint = m.clone();
    // Into lap 6: last two completed generations are enough to restore it.
    let mut flashback = packet(2026, 3, 20.0);
    if let Packet::Event(d) = &mut flashback.packet {
        d.details = pitwall_collector::telemetry::EventDetails::Flashback {
            frame_identifier: 0,
            session_time: 16.0,
        };
    }
    m.apply(Duration::from_secs(21), &flashback);
    let trace = m.car(0).unwrap().current_lap_trace().unwrap();
    assert_eq!(trace.lap_number, 6);
    assert_eq!(trace.samples.last().unwrap().distance_m, 10.0);
    assert!(trace.samples.iter().all(|s| s.session_time <= 16.0));
    point(&mut m, 6, 15.0, 1400, 3000, 16.4);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Previous),
        Readout::Live(-100.0)
    );
    assert_eq!(
        checkpoint
            .car(0)
            .unwrap()
            .current_lap_trace()
            .unwrap()
            .lap_number,
        7
    );
    // Past retained depth: summaries survive, but the named lap has no samples.
    if let Packet::Event(d) = &mut flashback.packet {
        d.details = pitwall_collector::telemetry::EventDetails::Flashback {
            frame_identifier: 0,
            session_time: 7.0,
        };
    }
    m.apply(Duration::from_secs(22), &flashback);
    point(&mut m, 3, 10.0, 1000, 3000, 7.0);
    assert_eq!(
        m.car(0)
            .unwrap()
            .reference_lap(Reference::Previous)
            .unwrap()
            .lap_number,
        2
    );
    assert!(m.car(0).unwrap().lap_trace(Reference::Previous).is_none());
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Previous),
        Readout::Absent
    );
}

#[test]
fn assumed_line_packet_with_nonzero_elapsed_uses_official_delta() {
    let mut m = SessionModel::default();
    point(&mut m, 1, 0.0, 0, 0, 0.0);
    point(&mut m, 1, 10.0, 1000, 0, 1.0);
    point(&mut m, 2, 0.0, 0, 2000, 2.0);
    point(&mut m, 2, 10.0, 900, 2000, 2.9);
    point(&mut m, 3, 1.0, 100, 1800, 3.9);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Best),
        Readout::Live(-200.0)
    );
    point(&mut m, 3, 5.0, 400, 1800, 4.2);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Best),
        Readout::Live(-50.0)
    );
}

#[test]
fn assumed_first_finish_has_no_self_reference_and_trace_channels_survive_both_formats() {
    for format in [2025, 2026] {
        let mut m = SessionModel::default();
        let mut lap_packet = packet(format, 2, 0.0);
        let mut telemetry = packet(format, 6, 0.0);
        for step in 0..=1 {
            telemetry.header.session_time = step as f32 * 0.01;
            if let Packet::CarTelemetry(d) = &mut telemetry.packet {
                for t in d.car_telemetry_data.iter_mut().flatten() {
                    t.speed = 100 + step * 100;
                    t.throttle = step as f32;
                    t.brake = 1.0 - step as f32;
                }
            }
            apply(&mut m, &telemetry);
            lap_packet.header.session_time = telemetry.header.session_time;
            if let Packet::Lap(d) = &mut lap_packet.packet {
                for l in d.lap_data.iter_mut().flatten() {
                    l.result_status = 2;
                    l.current_lap_num = 1;
                    l.lap_distance = step as f32 * 10.0;
                    l.current_lap_time_in_ms = u32::from(step) * 1000;
                }
            }
            apply(&mut m, &lap_packet);
        }
        let sample = &m.car(0).unwrap().current_lap_trace().unwrap().samples[1];
        assert_eq!(
            (sample.speed_kph, sample.throttle, sample.brake),
            (Some(150.0), Some(0.5), Some(0.5))
        );
        lap_packet.header.session_time = 2.0;
        if let Packet::Lap(d) = &mut lap_packet.packet {
            for l in d.lap_data.iter_mut().flatten() {
                l.current_lap_num = 2;
                l.lap_distance = 0.0;
                l.current_lap_time_in_ms = 0;
                l.last_lap_time_in_ms = 2000;
            }
        }
        apply(&mut m, &lap_packet);
        assert_eq!(
            m.car(0).unwrap().live_delta(Reference::Best),
            Readout::Absent
        );
        assert_eq!(
            m.car(0).unwrap().live_delta(Reference::Previous),
            Readout::Absent
        );
        lap_packet.header.session_time = 2.01;
        if let Packet::Lap(d) = &mut lap_packet.packet {
            for l in d.lap_data.iter_mut().flatten() {
                l.lap_distance = 5.0;
                l.current_lap_time_in_ms = 400;
            }
        }
        apply(&mut m, &lap_packet);
        assert_eq!(
            m.car(0).unwrap().live_delta(Reference::Best),
            Readout::Live(-100.0)
        );
        m.advance(Duration::from_secs(40));
        assert_eq!(
            m.car(0).unwrap().live_delta(Reference::Best),
            Readout::Stale(-100.0)
        );
        lap_packet.header.session_uid = 202;
        lap_packet.header.session_time = 0.0;
        m.apply(Duration::from_secs(41), &lap_packet);
        assert!(m.car(0).unwrap().reference_lap(Reference::Best).is_none());
    }
}

#[test]
fn assumed_former_best_survives_long_session_for_rewind_and_checkpoints_are_pure() {
    let mut m = SessionModel::default();
    point(&mut m, 1, 0.0, 0, 0, 0.0);
    point(&mut m, 1, 10.0, 1000, 0, 1.0);
    point(&mut m, 2, 0.0, 0, 3000, 3.0);
    point(&mut m, 2, 10.0, 900, 3000, 3.9);
    point(&mut m, 3, 0.0, 0, 2900, 5.9);
    for number in 4..=9 {
        point(
            &mut m,
            number,
            0.0,
            0,
            3100,
            5.9 + f32::from(number - 3) * 3.1,
        );
    }
    let checkpoint = m.clone();
    let mut rewind = packet(2026, 3, 30.0);
    if let Packet::Event(d) = &mut rewind.packet {
        d.details = pitwall_collector::telemetry::EventDetails::Flashback {
            frame_identifier: 0,
            session_time: 4.0,
        };
    }
    m.apply(Duration::from_secs(31), &rewind);
    assert_eq!(
        m.car(0)
            .unwrap()
            .reference_lap(Reference::Best)
            .unwrap()
            .lap_number,
        1
    );
    assert!(m.car(0).unwrap().lap_trace(Reference::Best).is_some());
    point(&mut m, 2, 5.0, 400, 3000, 4.0);
    assert_eq!(
        m.car(0).unwrap().live_delta(Reference::Best),
        Readout::Live(-100.0)
    );
    let mut replay = checkpoint.clone();
    replay.apply(Duration::from_secs(31), &rewind);
    point(&mut replay, 2, 5.0, 400, 3000, 4.0);
    assert_eq!(m, replay);
    assert_eq!(
        checkpoint
            .car(0)
            .unwrap()
            .reference_lap(Reference::Best)
            .unwrap()
            .lap_number,
        2
    );
}

#[test]
fn assumed_each_lap_data_update_reaches_the_frame_without_waiting_for_state() {
    use pitwall_collector::wire::{Message, Source, Stream};
    let mut model = SessionModel::default();
    point(&mut model, 1, 0., 0, 0, 0.);
    point(&mut model, 1, 10., 1000, 0, 1.);
    point(&mut model, 2, 0., 0, 2000, 2.);
    let stream = Stream::new("delta-test");
    let screen = stream.subscribe();
    for (i, (ms, expected)) in [(700, -50_i32), (701, -49), (702, -48)]
        .into_iter()
        .enumerate()
    {
        // No telemetry or State-rate clock advance is needed for the delta to change.
        point(&mut model, 2, 7.5, ms, 2000, 2.7);
        stream.tick(
            Duration::from_millis(17 * (i as u64 + 1)),
            Source::Live,
            &model,
            Default::default(),
        );
        let mut latest = None;
        while let Some(message) = screen.pop() {
            if let Message::Frame(bytes) = message {
                latest = Some(bytes);
            }
        }
        let bytes = latest.expect("each changed Lap Data sample must publish a Frame");
        assert_eq!(
            i32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            expected
        );
    }
}
