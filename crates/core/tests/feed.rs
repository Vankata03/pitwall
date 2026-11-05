//! The UI read API: a slow-state snapshot and a separate fast path for frame-rate data.
//! Assumed synthetic fixtures; packet-table fixtures are not capture evidence.
use pitwall_core::{
    feed::{self, CarInputs},
    model::{Lifecycle, Readout, SessionModel},
    telemetry::{ParsedDatagram, parse_datagram},
};
use std::{process::Command, time::Duration};

fn packet(id: u8, fields: &[(&str, &str)]) -> ParsedDatagram {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/telemetry/assumed_datagram.py"
    ));
    command.args(["2025".to_owned(), id.to_string()]);
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

#[test]
fn reader_starts_with_an_empty_snapshot_and_no_car_inputs() {
    let (_publisher, reader) = feed::channel();
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.generation, 0);
    assert_eq!(snapshot.lifecycle(), Lifecycle::Waiting);
    let fast = reader.fast();
    assert_eq!(fast.generation, 0);
    assert_eq!(fast.player_car_index, None);
    assert!(fast.cars.iter().all(|car| *car == Readout::Absent));
}

#[test]
fn trace_readers_receive_every_intervening_frame_in_publication_order() {
    let (mut publisher, reader) = feed::channel();
    let model = SessionModel::default();
    for _ in 0..3 {
        publisher.publish_fast(&model);
    }
    let batch = reader.fast_since(0);
    assert!(!batch.overrun);
    assert_eq!(
        batch
            .frames
            .iter()
            .map(|f| f.generation)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert!(reader.fast_since(3).frames.is_empty());
    publisher.publish_fast(&model);
    assert_eq!(reader.fast_since(3).frames[0].generation, 4);
}

#[test]
fn a_slow_trace_reader_is_told_when_bounded_history_overwrites_old_frames() {
    let (mut publisher, reader) = feed::channel();
    for _ in 0..259 {
        publisher.publish_fast(&SessionModel::default());
    }
    let batch = reader.fast_since(0);
    assert!(batch.overrun);
    assert_eq!(batch.frames.len(), 256);
    assert_eq!(batch.frames.first().unwrap().generation, 4);
    assert_eq!(batch.frames.last().unwrap().generation, 259);
    assert!(!reader.fast_since(259).overrun);
}

/// A model with car 0 valid and one Car Telemetry packet applied at `at`.
fn model_with_inputs(at: Duration, fields: &[(&str, &str)]) -> SessionModel {
    let mut model = SessionModel::default();
    model.apply(at, &packet(2, &[("lap_data.0.result_status", "2")]));
    model.apply(at, &packet(6, fields));
    model
}

#[test]
fn fast_path_carries_each_cars_inputs_with_their_staleness() {
    let (mut publisher, reader) = feed::channel();
    let mut model = model_with_inputs(
        Duration::ZERO,
        &[
            ("header.player_car_index", "0"),
            ("header.session_time", "12.5"),
            ("car_telemetry_data.0.speed", "287"),
            ("car_telemetry_data.0.throttle", "0.75"),
            ("car_telemetry_data.0.brake", "0.25"),
            ("car_telemetry_data.0.steer", "-0.5"),
            ("car_telemetry_data.0.clutch", "40"),
            ("car_telemetry_data.0.gear", "-1"),
            ("car_telemetry_data.0.engine_rpm", "11800"),
            ("car_telemetry_data.0.drs", "1"),
            ("car_telemetry_data.0.rev_lights_percent", "93"),
        ],
    );
    let expected = CarInputs {
        speed: 287,
        throttle: 0.75,
        brake: 0.25,
        steer: -0.5,
        clutch: 40,
        gear: -1,
        engine_rpm: 11800,
        drs: true,
        rev_lights_percent: 93,
    };

    publisher.publish_fast(&model);
    let frame = reader.fast();
    assert_eq!(frame.generation, 1);
    assert_eq!(reader.fast_generation(), 1);
    assert_eq!(frame.session_time, 12.5);
    assert_eq!(frame.player_car_index, Some(0));
    assert_eq!(frame.cars[0], Readout::Live(expected));
    assert!(frame.cars[1..].iter().all(|car| *car == Readout::Absent));

    // Three menu intervals of silence make the car's telemetry stale.
    model.advance(Duration::from_millis(150));
    publisher.publish_fast(&model);
    let frame = reader.fast();
    assert_eq!(frame.generation, 2);
    assert_eq!(frame.cars[0], Readout::Stale(expected));
}

#[test]
fn fast_path_carries_each_cars_lap_distance_with_its_own_staleness() {
    let (mut publisher, reader) = feed::channel();
    let mut model = SessionModel::default();
    model.apply(
        Duration::ZERO,
        &packet(
            2,
            &[
                ("lap_data.0.result_status", "2"),
                ("lap_data.0.lap_distance", "1234.5"),
                ("lap_data.1.result_status", "2"),
                ("lap_data.1.lap_distance", "-12.25"),
            ],
        ),
    );
    publisher.publish_fast(&model);
    let frame = reader.fast();
    assert_eq!(frame.lap_distances[0], Readout::Live(1234.5));
    assert_eq!(frame.lap_distances[1], Readout::Live(-12.25));
    assert_eq!(frame.lap_distances[2], Readout::Absent);
    // Lap distance has no telemetry yet: the inputs stay absent.
    assert_eq!(frame.cars[0], Readout::Absent);

    model.advance(Duration::from_millis(150));
    publisher.publish_fast(&model);
    assert_eq!(reader.fast().lap_distances[0], Readout::Stale(1234.5));
}

#[test]
fn fast_and_slow_paths_publish_independently() {
    let (mut publisher, reader) = feed::channel();
    let model = model_with_inputs(Duration::ZERO, &[("car_telemetry_data.0.speed", "200")]);

    publisher.publish_fast(&model);
    assert_eq!(reader.snapshot().generation, 0);
    assert!(reader.snapshot().car(0).is_none());

    publisher.publish_snapshot(&model);
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.lifecycle(), Lifecycle::Live);
    assert!(matches!(snapshot.car(0).unwrap().telemetry(), Readout::Live(t) if t.speed == 200));
    assert_eq!(reader.fast().generation, 1);

    // A held snapshot is immutable: later publishes do not change it.
    publisher.publish_snapshot(&SessionModel::default());
    assert_eq!(snapshot.lifecycle(), Lifecycle::Live);
    assert_eq!(reader.snapshot().generation, 2);
    assert_eq!(reader.snapshot().lifecycle(), Lifecycle::Waiting);
}

#[test]
fn concurrent_readers_never_see_a_torn_fast_frame() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let slow = model_with_inputs(
        Duration::ZERO,
        &[
            ("car_telemetry_data.0.speed", "100"),
            ("car_telemetry_data.0.throttle", "0.1"),
            ("car_telemetry_data.0.gear", "2"),
        ],
    );
    let fast = model_with_inputs(
        Duration::ZERO,
        &[
            ("car_telemetry_data.0.speed", "300"),
            ("car_telemetry_data.0.throttle", "0.9"),
            ("car_telemetry_data.0.gear", "7"),
        ],
    );
    let (mut publisher, reader) = feed::channel();
    publisher.publish_fast(&slow);
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let reader = reader.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut reads = 0u64;
                let mut last = 0;
                let mut trace_generation = 0;
                while !stop.load(Ordering::Relaxed) {
                    let frame = reader.fast();
                    assert!(frame.generation >= last, "generation went backwards");
                    last = frame.generation;
                    let Readout::Live(car) = frame.cars[0] else {
                        panic!("car 0 missing: {:?}", frame.cars[0]);
                    };
                    assert!(
                        matches!(
                            (car.speed, car.throttle, car.gear),
                            (100, 0.1, 2) | (300, 0.9, 7)
                        ),
                        "torn frame: {car:?}"
                    );
                    reads += 1;
                    for retained in reader.fast_since(trace_generation).frames {
                        assert!(retained.generation > trace_generation);
                        trace_generation = retained.generation;
                        let Readout::Live(car) = retained.cars[0] else {
                            panic!("retained car 0 missing");
                        };
                        let expected = if retained.generation.is_multiple_of(2) {
                            (300, 0.9, 7)
                        } else {
                            (100, 0.1, 2)
                        };
                        assert_eq!(
                            (car.speed, car.throttle, car.gear),
                            expected,
                            "retained frame must match its publication generation"
                        );
                    }
                }
                reads
            })
        })
        .collect();
    for i in 0..200_000 {
        publisher.publish_fast(if i % 2 == 0 { &fast } else { &slow });
    }
    stop.store(true, Ordering::Relaxed);
    for reader in readers {
        assert!(reader.join().unwrap() > 0);
    }
    assert_eq!(reader.fast().generation, 200_001);
}
