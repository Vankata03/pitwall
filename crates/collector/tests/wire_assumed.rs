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
