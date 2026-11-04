//! Synthetic wire inputs; assumed until qualified against captures.
use pitwall_collector::{
    model::SessionModel,
    wire::{Message, Source, Stream},
};
use std::time::Duration;
#[test]
fn assumed_connect_snapshot_and_virtual_clock_latest_frame() {
    let stream = Stream::new("test");
    let model = SessionModel::default();
    let screen = stream.subscribe();
    assert!(matches!(screen.pop(), Some(Message::Text(v)) if v["type"] == "hello"));
    assert!(matches!(screen.pop(), Some(Message::Text(v)) if v["type"] == "snapshot"));
    stream.tick(
        Duration::from_millis(17),
        Source::Live,
        &model,
        Default::default(),
    );
    assert!(matches!(screen.pop(), Some(Message::Text(v)) if v["type"] == "state"));
    let Some(Message::Frame(bytes)) = screen.pop() else {
        panic!("missing frame")
    };
    assert_eq!(bytes.len(), 976);
    assert_eq!(&bytes[54..56], &[255, 63]); // seven absent groups; reserved bits zero
    stream.tick(
        Duration::from_millis(34),
        Source::Live,
        &model,
        Default::default(),
    );
    assert!(screen.pop().is_none(), "unchanged frame suppressed");
}

fn packet(id: u8, time: f32) -> pitwall_collector::telemetry::ParsedDatagram {
    use std::process::Command;
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/telemetry/assumed_datagram.py"
        ))
        .args(["2026", &id.to_string(), "--set", "header.session_uid=101"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut p = pitwall_collector::telemetry::parse_datagram(&output.stdout).unwrap();
    p.header.session_time = time;
    p
}
fn drain(screen: &pitwall_collector::wire::Screen) -> Vec<serde_json::Value> {
    let mut out = vec![];
    while let Some(m) = screen.pop() {
        if let Message::Text(v) = m {
            out.push(v)
        }
    }
    out
}
#[test]
fn assumed_epochs_switch_source_seek_and_new_session() {
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let screen = s.subscribe();
    drain(&screen);
    m.apply(Duration::ZERO, &packet(1, 0.));
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let epoch = s.epoch();
    assert_eq!(drain(&screen)[0]["type"], "snapshot");
    s.tick(
        Duration::from_millis(17),
        Source::Replay,
        &m,
        Default::default(),
    );
    assert_eq!(s.epoch(), epoch + 1);
    assert_eq!(drain(&screen)[0]["source"], "replay");
    s.reset();
    s.tick(
        Duration::from_millis(34),
        Source::Replay,
        &m,
        Default::default(),
    );
    assert_eq!(s.epoch(), epoch + 2);
    let mut p = packet(1, 1.);
    p.header.session_uid = 202;
    m.apply(Duration::from_secs(1), &p);
    s.tick(
        Duration::from_secs(1),
        Source::Replay,
        &m,
        Default::default(),
    );
    assert_eq!(s.epoch(), epoch + 3);
}
#[test]
fn assumed_history_corrections_replace_same_id_without_epoch_reset() {
    use pitwall_collector::telemetry::Packet;
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut valid = packet(2, 0.);
    if let Packet::Lap(l) = &mut valid.packet {
        for v in l.lap_data.iter_mut().flatten() {
            v.result_status = 2;
            v.current_lap_num = 2;
        }
    }
    m.apply(Duration::ZERO, &valid);
    let mut p = packet(11, 1.);
    if let Packet::SessionHistory(h) = &mut p.packet {
        h.car_idx = 0;
        h.num_laps = 1;
        h.lap_history_data[0].lap_time_in_ms = 90000;
        h.lap_history_data[0].lap_valid_bit_flags = 15;
    }
    m.apply(Duration::ZERO, &p);
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let screen = s.subscribe();
    let initial = drain(&screen);
    let lap = initial[1]["history"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "lap")
        .unwrap();
    let id = lap["id"].clone();
    let epoch = s.epoch();
    if let Packet::SessionHistory(h) = &mut p.packet {
        h.lap_history_data[0].lap_time_in_ms = 91000;
    }
    m.apply(Duration::from_millis(10), &p);
    s.tick(
        Duration::from_millis(10),
        Source::Live,
        &m,
        Default::default(),
    );
    let messages = drain(&screen);
    let entries = messages[0]["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e["id"] == id && e["body"]["lap_time_ms"] == 91000)
    );
    assert_eq!(s.epoch(), epoch);
}
#[test]
fn assumed_history_overflow_closes_only_slow_screen() {
    use pitwall_collector::telemetry::{EventDetails, Packet};
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut p = packet(3, 0.);
    if let Packet::Event(e) = &mut p.packet {
        e.details = EventDetails::FastestLap {
            vehicle_idx: 0,
            lap_time: 90.,
        };
    }
    m.apply(Duration::ZERO, &p);
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let slow = s.subscribe();
    drain(&slow);
    for n in 1..=4097 {
        p.header.session_time = n as f32;
        m.apply(Duration::from_secs(n), &p);
    }
    s.tick(
        Duration::from_secs(4097),
        Source::Live,
        &m,
        Default::default(),
    );
    assert!(slow.closed());
    let fresh = s.subscribe();
    assert!(!fresh.closed());
    assert_eq!(drain(&fresh)[1]["history"].as_array().unwrap().len(), 4098);
}
#[test]
fn assumed_frame_table_scaling_latest_queues_and_state_revisions() {
    use pitwall_collector::{
        telemetry::Packet,
        wire::{Context, Revisions},
    };
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut lap = packet(2, 0.);
    if let Packet::Lap(l) = &mut lap.packet {
        for v in l.lap_data.iter_mut().flatten() {
            v.result_status = 2;
            v.current_lap_num = 1;
            v.lap_distance = 10.;
            v.current_lap_time_in_ms = 1234;
        }
    }
    m.apply(Duration::ZERO, &lap);
    let mut p = packet(6, 1.);
    if let Packet::CarTelemetry(t) = &mut p.packet {
        let t = t.car_telemetry_data[0].as_mut().unwrap();
        t.speed = 321;
        t.gear = -1;
        t.throttle = 0.75;
        t.brake = 0.25;
        t.steer = -0.5;
        t.engine_rpm = 12000;
    }
    m.apply(Duration::ZERO, &p);
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let screen = s.subscribe();
    drain(&screen);
    for n in 1..=20 {
        p.header.session_time = n as f32;
        m.apply(Duration::from_millis(n * 20), &p);
        s.tick(
            Duration::from_millis(n * 20),
            Source::Live,
            &m,
            Context {
                revisions: Revisions {
                    layouts: n as u32,
                    tracks: 7,
                },
                ..Default::default()
            },
        );
    }
    let state = screen.pop().unwrap();
    assert!(matches!(state,Message::Text(v) if v["context"]["revisions"]["layouts"]==20));
    let Some(Message::Frame(bytes)) = screen.pop() else {
        panic!("frame")
    };
    assert_eq!(&bytes[16..24], &[65, 1, 255, 75, 25, 206, 224, 46]);
    assert_eq!(f64::from_le_bytes(bytes[8..16].try_into().unwrap()), 20.);
    assert!(screen.pop().is_none());
}
#[test]
fn assumed_lap_trace_columns_and_stale_epoch() {
    use pitwall_collector::telemetry::Packet;
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut p = packet(2, 0.);
    for (distance, time) in [(0., 0), (10., 1000)] {
        if let Packet::Lap(l) = &mut p.packet {
            for v in l.lap_data.iter_mut().flatten() {
                v.result_status = 2;
                v.current_lap_num = 1;
                v.lap_distance = distance;
                v.current_lap_time_in_ms = time;
            }
        }
        p.header.session_time = time as f32 / 1000.;
        m.apply(Duration::from_millis(time as u64), &p);
    }
    s.tick(Duration::from_secs(1), Source::Live, &m, Default::default());
    let trace = s.trace(0, 1, s.epoch()).unwrap();
    assert_eq!(trace.distance, vec![0., 5., 10.]);
    assert_eq!(trace.time, vec![0., 500., 1000.]);
    assert_eq!(trace.speed, vec![None, None, None]);
    assert_eq!(s.trace(0, 1, s.epoch() + 1).unwrap_err(), 409);
    assert_eq!(s.trace(24, 1, s.epoch()).unwrap_err(), 404);
}
#[test]
fn assumed_late_join_receives_current_frame_even_when_unchanged() {
    let s = Stream::new("test");
    s.tick(
        Duration::ZERO,
        Source::Live,
        &SessionModel::default(),
        Default::default(),
    );
    let screen = s.subscribe();
    screen.pop();
    screen.pop();
    assert!(matches!(screen.pop(), Some(Message::Frame(_))));
}
#[test]
fn assumed_race_control_snapshot_preserves_arrival_order() {
    use pitwall_collector::telemetry::{EventDetails, Packet};
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut p = packet(3, 0.);
    if let Packet::Event(e) = &mut p.packet {
        e.details = EventDetails::FastestLap {
            vehicle_idx: 0,
            lap_time: 90.,
        };
    }
    for n in 0..12 {
        p.header.session_time = n as f32;
        m.apply(Duration::from_secs(n), &p);
    }
    s.tick(
        Duration::from_secs(12),
        Source::Live,
        &m,
        Default::default(),
    );
    let snapshot = drain(&s.subscribe());
    let entries = snapshot[1]["history"].as_array().unwrap();
    let times: Vec<_> = entries
        .iter()
        .filter(|e| e["kind"] == "raceControl")
        .map(|e| e["body"]["session_time"].as_f64().unwrap() as u32)
        .collect();
    assert_eq!(times, (0..12).collect::<Vec<_>>());
}
#[test]
fn assumed_flashback_snapshots_and_frame_stale_restricted_bits() {
    use pitwall_collector::telemetry::{EventDetails, Packet};
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut lap = packet(2, 10.);
    if let Packet::Lap(l) = &mut lap.packet {
        for v in l.lap_data.iter_mut().flatten() {
            v.result_status = 2;
        }
    }
    m.apply(Duration::ZERO, &lap);
    m.apply(Duration::ZERO, &packet(6, 10.));
    let mut participants = packet(4, 10.);
    if let Packet::Participants(p) = &mut participants.packet {
        p.num_active_cars = 24;
    }
    m.apply(Duration::ZERO, &participants);
    m.advance(Duration::from_millis(60));
    s.tick(
        Duration::from_millis(60),
        Source::Live,
        &m,
        Default::default(),
    );
    let epoch = s.epoch();
    let screen = s.subscribe();
    screen.pop();
    screen.pop();
    let Some(Message::Frame(bytes)) = screen.pop() else {
        panic!("frame")
    };
    let bits = u16::from_le_bytes(bytes[54..56].try_into().unwrap());
    assert_eq!(bits & 3, 1);
    assert_eq!((bits >> 6) & 3, 2);
    let mut flash = packet(3, 11.);
    if let Packet::Event(e) = &mut flash.packet {
        e.details = EventDetails::Flashback {
            frame_identifier: 1,
            session_time: 5.,
        };
    }
    m.apply(Duration::from_millis(61), &flash);
    s.tick(
        Duration::from_millis(61),
        Source::Live,
        &m,
        Default::default(),
    );
    assert_eq!(s.epoch(), epoch + 1);
    assert_eq!(drain(&screen)[0]["type"], "snapshot");
}

#[test]
fn assumed_flashback_frame_uses_target_session_time() {
    use pitwall_collector::telemetry::{EventDetails, Packet};
    let mut m = SessionModel::default();
    let mut p = packet(3, 11.);
    if let Packet::Event(e) = &mut p.packet {
        e.details = EventDetails::Flashback {
            frame_identifier: 1,
            session_time: 5.,
        };
    }
    m.apply(Duration::ZERO, &p);
    let bytes = pitwall_collector::wire::frame::encode(&m, 1, 1);
    assert_eq!(f64::from_le_bytes(bytes[8..16].try_into().unwrap()), 5.);
}

#[test]
fn assumed_session_total_laps_reaches_state() {
    use pitwall_collector::{
        telemetry::Packet,
        wire::{Context, State},
    };
    let mut p = packet(1, 0.);
    if let Packet::Session(ref mut s) = p.packet {
        s.total_laps = 44;
    } else {
        panic!("expected session packet");
    }
    let mut model = SessionModel::default();
    model.apply(Duration::ZERO, &p);
    let value = serde_json::to_value(State::capture(&model, Context::default())).unwrap();
    assert_eq!(value["session"]["value"]["totalLaps"], 44);
}

#[test]
fn assumed_history_removal_replaces_summaries_without_resetting_live_frames() {
    use pitwall_collector::telemetry::Packet;
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut lap = packet(2, 0.);
    if let Packet::Lap(l) = &mut lap.packet {
        l.lap_data[0].as_mut().unwrap().result_status = 2;
    }
    m.apply(Duration::ZERO, &lap);
    let mut p = packet(11, 1.);
    if let Packet::SessionHistory(h) = &mut p.packet {
        h.car_idx = 0;
        h.num_tyre_stints = 3;
        for (index, stint) in h.tyre_stints_history_data.iter_mut().take(3).enumerate() {
            stint.end_lap = (index as u8 + 1) * 3;
            stint.tyre_actual_compound = 16;
        }
    }
    m.apply(Duration::ZERO, &p);
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let screen = s.subscribe();
    drain(&screen);
    let epoch = s.epoch();
    if let Packet::SessionHistory(h) = &mut p.packet {
        h.num_tyre_stints = 2;
    }
    m.apply(Duration::from_millis(10), &p);
    s.tick(
        Duration::from_millis(10),
        Source::Live,
        &m,
        Default::default(),
    );
    assert_eq!(s.epoch(), epoch);
    let messages = drain(&screen);
    assert!(!messages.iter().any(|m| m["type"] == "snapshot"));
    let correction = messages.iter().find(|m| m["type"] == "history").unwrap();
    assert_eq!(correction["removed"].as_array().unwrap().len(), 1);
    assert!(correction.to_string().len() < 4096);
}

#[test]
fn assumed_twenty_hz_arrivals_stay_live_for_three_game_intervals() {
    use pitwall_collector::model::Readout;
    let mut m = SessionModel::default();
    let mut lap = packet(2, 0.);
    if let pitwall_collector::telemetry::Packet::Lap(l) = &mut lap.packet {
        l.lap_data[0].as_mut().unwrap().result_status = 2;
    }
    m.apply(Duration::ZERO, &lap);
    for n in 0..20 {
        let mut p = packet(6, n as f32 * 0.05);
        p.header.frame_identifier = n;
        m.apply(Duration::from_millis(u64::from(n) * 50), &p);
    }
    m.advance(Duration::from_millis(1050));
    assert!(matches!(m.car(0).unwrap().telemetry(), Readout::Live(_)));
    m.advance(Duration::from_millis(1100));
    assert!(matches!(m.car(0).unwrap().telemetry(), Readout::Stale(_)));
}

#[test]
fn assumed_stint_wear_update_republishes_only_the_changed_entry() {
    use pitwall_collector::telemetry::Packet;
    // Car Damage arrives about ten times a second and moves the current stint's wear.
    // Unchanged laps and other groups stay published without being re-sent.
    let s = Stream::new("test");
    let mut m = SessionModel::default();
    let mut lap = packet(2, 0.);
    if let Packet::Lap(l) = &mut lap.packet {
        for v in l.lap_data.iter_mut().flatten() {
            v.result_status = 2;
            v.current_lap_num = 2;
        }
    }
    m.apply(Duration::ZERO, &lap);
    let mut history = packet(11, 1.);
    if let Packet::SessionHistory(h) = &mut history.packet {
        h.car_idx = 0;
        h.num_laps = 1;
        h.lap_history_data[0].lap_time_in_ms = 90000;
        h.lap_history_data[0].lap_valid_bit_flags = 15;
    }
    m.apply(Duration::ZERO, &history);
    let mut participants = packet(4, 1.);
    if let Packet::Participants(p) = &mut participants.packet {
        p.num_active_cars = p.participants.len() as u8;
        for v in p.participants.iter_mut().flatten() {
            v.your_telemetry = 1;
        }
    }
    m.apply(Duration::ZERO, &participants);
    m.apply(Duration::ZERO, &packet(7, 1.));
    let mut damage = packet(10, 1.);
    m.apply(Duration::ZERO, &damage);
    s.tick(Duration::ZERO, Source::Live, &m, Default::default());
    let screen = s.subscribe();
    let snapshot = drain(&screen);
    let entries = snapshot[1]["history"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["kind"] == "lap"));
    assert!(
        entries
            .iter()
            .any(|e| e["kind"] == "stint" && e["car"] == 0)
    );

    if let Packet::CarDamage(d) = &mut damage.packet {
        d.car_damage_data[0].as_mut().unwrap().tyres_wear[0] += 7.0;
    }
    damage.header.session_time = 2.;
    m.apply(Duration::from_millis(10), &damage);
    s.tick(
        Duration::from_millis(10),
        Source::Live,
        &m,
        Default::default(),
    );
    let messages = drain(&screen);
    let update = messages.iter().find(|m| m["type"] == "history").unwrap();
    let changed = update["entries"].as_array().unwrap();
    assert!(!changed.is_empty());
    assert!(
        changed
            .iter()
            .all(|e| e["kind"] == "stint" && e["car"] == 0),
        "{changed:?}"
    );
    assert!(update["removed"].as_array().unwrap().is_empty());

    // Same wear again: only unserialized bookkeeping moves, so nothing is published.
    damage.header.session_time = 3.;
    m.apply(Duration::from_millis(20), &damage);
    s.tick(
        Duration::from_millis(20),
        Source::Live,
        &m,
        Default::default(),
    );
    assert!(!drain(&screen).iter().any(|m| m["type"] == "history"));

    let late = s.subscribe();
    let late_entries = drain(&late)[1]["history"].as_array().unwrap().clone();
    assert_eq!(late_entries.len(), entries.len());
    assert!(
        late_entries
            .iter()
            .any(|e| e["kind"] == "lap" && e["body"]["lap_time_ms"] == 90000)
    );
}
