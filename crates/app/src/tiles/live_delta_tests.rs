//! Live delta built from a model that drove laps through synthetic Lap Data.

use super::live_delta::*;
use pitwall_core::{
    model::{Readout, SessionModel},
    telemetry::{Packet, ParsedDatagram, parse_datagram},
};
use std::time::Duration;

pub(super) fn assumed(id: u8) -> ParsedDatagram {
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
pub(super) fn drive(to: u32) -> SessionModel {
    drive_with(to, |_| {})
}

fn drive_with(to: u32, mut observe: impl FnMut(&SessionModel)) -> SessionModel {
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
    let mut telemetry = assumed(6);
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
            telemetry.header.session_time = packet.header.session_time;
            if let Packet::CarTelemetry(data) = &mut telemetry.packet {
                for car in data.car_telemetry_data.iter_mut().flatten() {
                    car.throttle = f32::from(lap) * 0.2;
                    car.brake = 0.0;
                }
            }
            model.apply(
                Duration::from_secs_f32(packet.header.session_time),
                &telemetry,
            );
            model.apply(Duration::from_secs_f32(packet.header.session_time), &packet);
            observe(&model);
        }
        clock += (pace * 1000) as f32 / 1000.0;
        last = pace * 1000;
    }
    model
}

#[test]
fn delta_consumes_feed_samples_and_keeps_the_lap_beyond_feed_rollover() {
    let (mut publisher, reader) = pitwall_core::feed::channel();
    let mut memory = Memory::default();
    let mut observed = None;
    let model = drive_with(500, |model| {
        publisher.publish_fast(model);
        observed = Some(memory.inputs(&reader, model, Some(0), Reference::Best, (600.0, 300.0)));
    });
    let inputs = observed.unwrap();
    let at = |points: &[Point], d| points.iter().find(|p| p.distance == d).unwrap().delta;
    assert_eq!(at(&inputs.current, 400.0), 2.0);
    assert_eq!(at(&inputs.previous, 800.0), -4.0);
    // Repeated publications roll over the feed, but do not duplicate lap positions.
    for _ in 0..(pitwall_core::feed::FAST_HISTORY_CAPACITY + 1) {
        publisher.publish_fast(&model);
    }
    let inputs = memory.inputs(&reader, &model, Some(0), Reference::Best, (600.0, 300.0));
    assert_eq!(at(&inputs.current, 400.0), 2.0);
    let mut late = Memory::default();
    let joined = late.inputs(&reader, &model, Some(0), Reference::Best, (600.0, 300.0));
    assert_eq!(
        joined
            .current
            .iter()
            .filter(|p| p.delta.is_finite())
            .map(|p| p.distance)
            .collect::<Vec<_>>(),
        [475.0]
    );
    assert!(joined.previous.is_empty());
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

#[test]
fn opening_delta_midlap_does_not_reconstruct_unobserved_model_history() {
    let model = drive(500);
    let inputs = Inputs::from_model(&model, Some(0), Reference::Best, (600.0, 300.0));
    assert!(
        inputs.current.is_empty(),
        "the model supplies the headline, not historical graph vertices"
    );
    assert!(inputs.previous.is_empty());
}

#[test]
fn fast_feed_preserves_named_deltas_independently_of_slow_state() {
    let model = drive(500);
    let (mut publisher, reader) = pitwall_core::feed::channel();
    publisher.publish_fast(&model);
    assert_eq!(reader.snapshot().generation, 0);
    let frame = reader.fast();
    assert_eq!(frame.delta_best[0], Readout::Live(2375.0));
    assert_eq!(frame.delta_previous[0], Readout::Live(2375.0));
    assert_eq!(reader.fast_since(0).frames[0], frame);
}

struct Stream {
    model: SessionModel,
    publisher: pitwall_core::feed::Publisher,
    reader: pitwall_core::feed::Reader,
    lap: ParsedDatagram,
    clock: f32,
}
impl Stream {
    fn new() -> Self {
        let model = drive(500);
        let (mut publisher, reader) = pitwall_core::feed::channel();
        publisher.publish_fast(&model);
        Self {
            model,
            publisher,
            reader,
            lap: assumed(2),
            clock: 174.0,
        }
    }
    fn add(&mut self, number: u8, distance: f32, time_ms: u32) {
        self.clock += 0.01;
        self.lap.header.session_time = self.clock;
        self.lap.header.overall_frame_identifier += 1;
        let Packet::Lap(data) = &mut self.lap.packet else {
            unreachable!()
        };
        let car = data.lap_data[0].as_mut().unwrap();
        car.result_status = 2;
        car.current_lap_num = number;
        car.lap_distance = distance;
        car.current_lap_time_in_ms = time_ms;
        car.last_lap_time_in_ms = 50_000;
        car.current_lap_invalid = 0;
        car.pit_status = 0;
        car.sector1_time_ms_part = 0;
        car.sector2_time_ms_part = 0;
        self.model
            .apply(Duration::from_secs_f32(self.clock), &self.lap);
        self.publisher.publish_fast(&self.model);
    }
    fn draw(&self, memory: &mut Memory, car: Option<usize>, reference: Reference) -> Inputs {
        memory.inputs(&self.reader, &self.model, car, reference, (600.0, 300.0))
    }
}
fn distances(points: &[Point]) -> Vec<Vec<f32>> {
    segments(points)
        .iter()
        .map(|s| s.iter().map(|p| p.distance).collect())
        .collect()
}

#[test]
fn official_finish_is_headline_only_including_repeated_first_next_lap_positions() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    stream.draw(&mut memory, Some(0), Reference::Best);
    stream.add(4, 500.0, 25_000);
    stream.add(4, 600.0, 30_000);
    stream.add(5, 2.0, 100);
    for _ in 0..3 {
        stream.publisher.publish_fast(&stream.model);
    }
    let line = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(line.delta, Readout::Live(5_000.0));
    assert!(line.current.is_empty());
    assert_eq!(distances(&line.previous), [vec![475.0, 500.0, 600.0]]);
    stream.add(5, 40.0, 1800);
    let next = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(distances(&next.current), [vec![40.0]]);
    assert!(next.current[0].delta.abs() < 1.0);
}

#[test]
fn delta_breaks_missing_values_negative_distances_short_reversals_and_large_gaps() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    stream.draw(&mut memory, Some(0), Reference::Best);
    for (d, ms) in [
        (500.0, 25000),
        (490.0, 25100),
        (520.0, 26000),
        (625.0, 31250),
        (-5.0, 31500),
        (650.0, 32500),
        (999.0, 49950),
    ] {
        stream.add(4, d, ms);
    }
    let inputs = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(
        distances(&inputs.current),
        [vec![475.0, 500.0], vec![520.0], vec![625.0], vec![650.0]]
    );
    assert!(
        inputs
            .current
            .iter()
            .all(|p| p.distance.is_nan() || p.distance >= 0.0)
    );
}

#[test]
fn overflow_keeps_old_lap_samples_but_breaks_before_the_retained_tail() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    stream.draw(&mut memory, Some(0), Reference::Best);
    stream.add(4, 500.0, 25000);
    stream.draw(&mut memory, Some(0), Reference::Best);
    for _ in 0..(pitwall_core::feed::FAST_HISTORY_CAPACITY + 40) {
        stream.add(4, 525.0, 26250);
    }
    stream.add(4, 550.0, 27500);
    let inputs = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(
        distances(&inputs.current),
        [vec![475.0, 500.0], vec![525.0, 550.0]]
    );
    let again = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(distances(&again.current), distances(&inputs.current));
}

#[test]
fn changing_car_or_reference_restarts_from_retained_observations_and_epoch_discards_old_laps() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    stream.draw(&mut memory, Some(0), Reference::Best);
    for _ in 0..(pitwall_core::feed::FAST_HISTORY_CAPACITY + 40) {
        stream.add(4, 500.0, 25000);
    }
    stream.draw(&mut memory, Some(0), Reference::Best);
    let changed = stream.draw(&mut memory, Some(0), Reference::Previous);
    assert_eq!(distances(&changed.current), [vec![500.0]]);
    let missing = stream.draw(&mut memory, None, Reference::Previous);
    assert!(missing.current.is_empty() && missing.previous.is_empty());
    let car = stream.draw(&mut memory, Some(1), Reference::Best);
    assert!(car.current.iter().all(|p| p.delta.is_nan()));
    // A backwards clock triggers the model's timeline reset as it does for replay/flashback.
    stream.clock = 170.0;
    let old_epoch = stream.model.epoch();
    stream.add(4, 350.0, 17500);
    assert!(stream.model.epoch() > old_epoch);
    let reset = stream.draw(&mut memory, Some(0), Reference::Best);
    assert!(reset.previous.is_empty());
    assert_eq!(distances(&reset.current), [vec![350.0]]);
}

#[test]
fn stale_delta_keeps_observed_graph_muted_and_waiting_and_lost_explain_absence() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    let initial = stream.draw(&mut memory, Some(0), Reference::Best);
    assert!(!view(&initial).chart.unwrap().muted);
    stream.model.advance(Duration::from_secs(1000));
    stream.publisher.publish_fast(&stream.model);
    let stale = view(&stream.draw(&mut memory, Some(0), Reference::Best));
    assert_eq!(
        stale.headline.state,
        pitwall_core::readout::ReadoutState::Stale
    );
    assert!(stale.chart.unwrap().muted);
    let absent = view(&stream.draw(&mut memory, None, Reference::Best));
    assert_eq!(absent.message, Some("No data: game paused or closed"));
    let (publisher, reader) = pitwall_core::feed::channel();
    let waiting = view(&Memory::default().inputs(
        &reader,
        &SessionModel::default(),
        None,
        Reference::Best,
        (600.0, 300.0),
    ));
    assert_eq!(waiting.message, Some("Waiting for the game"));
    drop(publisher);
}

fn input_view(
    stream: &Stream,
    memory: &mut super::throttle_brake_traces::Memory,
    reference: Reference,
) -> super::throttle_brake_traces::View {
    let fast = stream.reader.fast();
    super::throttle_brake_traces::view(&memory.inputs(
        &stream.reader,
        &fast,
        &stream.model,
        Some(0),
        &serde_json::json!({"reference": reference.key()}),
        pitwall_core::units::Units::DEFAULT,
        (700.0, 400.0),
    ))
}

#[test]
fn input_epoch_mismatch_clears_history_and_reference_before_the_model_catches_up() {
    use super::throttle_brake_traces as inputs;
    let mut stream = Stream::new();
    let mut memory = inputs::Memory::default();
    // Provide telemetry so there is current input history as well as a reference.
    stream.model.apply(Duration::from_secs(174), &assumed(6));
    stream.publisher.publish_fast(&stream.model);
    assert_eq!(
        input_view(&stream, &mut memory, Reference::Best).plot.x,
        [0.0]
    );
    let old = stream.model.clone();
    let mut reset = assumed(2);
    reset.header.session_uid = 102;
    stream.model.apply(Duration::from_secs(175), &reset);
    stream.publisher.publish_fast(&stream.model);
    let fast = stream.reader.fast();
    let v = inputs::view(&memory.inputs(
        &stream.reader,
        &fast,
        &old,
        Some(0),
        &serde_json::json!({}),
        pitwall_core::units::Units::DEFAULT,
        (700.0, 400.0),
    ));
    assert!(
        v.plot.x.is_empty(),
        "an old epoch must not survive a reset with no telemetry"
    );
    assert!(
        v.plot
            .series
            .iter()
            .all(|s| s.lap != pitwall_core::traces::Lap::Reference)
    );
}

#[test]
fn input_overflow_breaks_even_when_lost_publications_span_less_than_half_a_second() {
    let mut stream = Stream::new();
    let mut memory = super::throttle_brake_traces::Memory::default();
    let initial = input_view(&stream, &mut memory, Reference::Best);
    assert_eq!(initial.plot.x, [0.0]);
    for _ in 0..(pitwall_core::feed::FAST_HISTORY_CAPACITY + 40) {
        stream.publisher.publish_fast(&stream.model);
    }
    stream.add(4, 500.0, 25000);
    let v = input_view(&stream, &mut memory, Reference::Best);
    assert!(
        v.plot.series.iter().all(|s| s.values.contains(&None)),
        "both current and reference must break across feed loss"
    );
}

#[test]
fn input_car_switch_without_a_new_publication_starts_from_the_latest_observation() {
    use super::throttle_brake_traces as inputs;
    let stream = Stream::new();
    let mut memory = inputs::Memory::default();
    input_view(&stream, &mut memory, Reference::Best);
    let fast = stream.reader.fast();
    let v = inputs::view(&memory.inputs(
        &stream.reader,
        &fast,
        &stream.model,
        Some(1),
        &serde_json::json!({}),
        pitwall_core::units::Units::DEFAULT,
        (700.0, 400.0),
    ));
    assert_eq!(v.plot.x, [0.0]);
    assert_eq!(v.meters[0].value.text, "80%");
}

fn reference_throttle(v: &super::throttle_brake_traces::View) -> Option<f64> {
    v.plot
        .series
        .iter()
        .find(|s| {
            s.lap == pitwall_core::traces::Lap::Reference
                && s.channel == pitwall_core::traces::Channel::Throttle
        })
        .and_then(|s| s.values.last().copied().flatten())
}

#[test]
fn shared_model_completion_and_best_correction_replace_references_without_slow_state_or_cache() {
    let mut stream = Stream::new();
    let mut memory = super::throttle_brake_traces::Memory::default();
    stream.publisher.publish_snapshot(&stream.model);
    let held = stream.reader.snapshot();
    let before = input_view(&stream, &mut memory, Reference::Best);
    assert!((reference_throttle(&before).unwrap() - 60.0).abs() < 1e-4);
    let partial = pitwall_core::lap_traces::model_source(&stream.model, 0, 4).unwrap();
    assert_eq!(partial.trace.samples.last().unwrap().distance_m, 475.0);
    stream.add(4, 500.0, 25000);
    stream.add(4, 600.0, 30000);
    stream.add(5, 40.0, 1800);
    let complete = pitwall_core::lap_traces::model_source(&stream.model, 0, 4).unwrap();
    assert_eq!(complete.trace.samples.last().unwrap().distance_m, 600.0);
    assert_eq!(
        partial.trace.samples.last().unwrap().distance_m,
        475.0,
        "held snapshots stay immutable"
    );
    assert_eq!(stream.reader.snapshot().generation, held.generation);
    assert_eq!(
        held.car(0)
            .unwrap()
            .reference_lap(Reference::Previous)
            .unwrap()
            .lap_number,
        3
    );
    let previous = input_view(&stream, &mut memory, Reference::Previous);
    assert!((reference_throttle(&previous).unwrap() - 80.0).abs() < 1e-4);
    assert_eq!(
        stream
            .model
            .car(0)
            .unwrap()
            .reference_lap(Reference::Previous)
            .unwrap()
            .lap_number,
        4
    );

    // A same-epoch authority correction invalidates best lap 3. Best becomes lap 1.
    let epoch = stream.model.epoch();
    let mut correction = assumed(11);
    correction.header.session_time = stream.clock;
    let Packet::SessionHistory(data) = &mut correction.packet else {
        unreachable!()
    };
    data.car_idx = 0;
    data.num_laps = 4;
    for (i, (ms, valid)) in [
        (50_000, true),
        (55_000, true),
        (45_000, false),
        (50_000, true),
    ]
    .into_iter()
    .enumerate()
    {
        data.lap_history_data[i].lap_time_in_ms = ms;
        data.lap_history_data[i].lap_valid_bit_flags = if valid { 15 } else { 0 };
    }
    stream
        .model
        .apply(Duration::from_secs_f32(stream.clock), &correction);
    assert_eq!(stream.model.epoch(), epoch);
    let corrected = input_view(&stream, &mut memory, Reference::Best);
    assert!((reference_throttle(&corrected).unwrap() - 20.0).abs() < 1e-4);
    assert_eq!(stream.reader.snapshot().generation, held.generation);
    // Restoring validity makes the same named trace available again, without a retry cache.
    let Packet::SessionHistory(data) = &mut correction.packet else {
        unreachable!()
    };
    data.lap_history_data[2].lap_valid_bit_flags = 15;
    stream
        .model
        .apply(Duration::from_secs_f32(stream.clock), &correction);
    let restored = input_view(&stream, &mut memory, Reference::Best);
    assert!((reference_throttle(&restored).unwrap() - 60.0).abs() < 1e-4);
}

#[test]
fn late_input_join_keeps_observed_time_only_and_silence_marks_the_same_trace_stale() {
    let mut stream = Stream::new();
    for _ in 0..400 {
        stream.publisher.publish_fast(&stream.model);
    }
    let mut memory = super::throttle_brake_traces::Memory::default();
    let v = input_view(&stream, &mut memory, Reference::Best);
    assert_eq!(v.plot.x, [0.0]);
    assert_eq!(v.meters[0].value.text, "80%");
    stream.model.advance(Duration::from_secs(1000));
    stream.publisher.publish_fast(&stream.model);
    let stale = input_view(&stream, &mut memory, Reference::Best);
    assert_eq!(stale.status, Some("Stale \u{25F7}"));
    assert_eq!(stale.opacity, 0.55);
    assert_eq!(stale.plot.x, v.plot.x);
}

#[test]
fn opening_a_tile_uses_the_recent_feed_tail_instead_of_the_whole_historical_lap() {
    let mut stream = Stream::new();
    stream.clock = 184.0;
    stream.add(4, 500.0, 25000);
    let mut memory = Memory::default();
    let v = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(
        distances(&v.current),
        [vec![500.0]],
        "the web ring retains ten seconds at 60 Hz"
    );
}

#[test]
fn named_reference_unavailable_until_completion_is_retried_from_shared_model_without_fallback() {
    let mut stream = Stream::new();
    let mut memory = super::throttle_brake_traces::Memory::default();
    let mut completion = assumed(11);
    completion.header.session_time = stream.clock;
    let Packet::SessionHistory(data) = &mut completion.packet else {
        unreachable!()
    };
    data.car_idx = 0;
    data.num_laps = 4;
    data.lap_history_data[3].lap_time_in_ms = 44_000;
    data.lap_history_data[3].sector3_time_ms_part = 1000;
    data.lap_history_data[3].lap_valid_bit_flags = 15;
    stream
        .model
        .apply(Duration::from_secs_f32(stream.clock), &completion);
    let absent = input_view(&stream, &mut memory, Reference::Best);
    assert_eq!(
        reference_throttle(&absent),
        None,
        "do not fall back to best lap 3 while named lap 4 has only a partial trace"
    );
    stream.add(5, 40.0, 1800);
    let ready = input_view(&stream, &mut memory, Reference::Best);
    assert!((reference_throttle(&ready).unwrap() - 80.0).abs() < 1e-4);
    assert_eq!(stream.reader.snapshot().generation, 0);
}

#[test]
fn repeated_position_replaces_its_delta_and_fast_feed_keeps_best_and_previous_distinct() {
    let mut stream = Stream::new();
    let mut memory = Memory::default();
    stream.draw(&mut memory, Some(0), Reference::Best);
    stream.add(4, 500.0, 25000);
    stream.add(4, 500.0, 25100);
    let inputs = stream.draw(&mut memory, Some(0), Reference::Best);
    assert_eq!(distances(&inputs.current), [vec![475.0, 500.0]]);
    assert_eq!(inputs.current.last().unwrap().delta, 2.6);
    let (mut publisher, reader) = pitwall_core::feed::channel();
    drive_with(0, |model| {
        let car = model.car(0).unwrap();
        if car
            .lap()
            .value()
            .is_some_and(|l| l.current_lap_num == 3 && l.lap_distance == 400.0)
        {
            publisher.publish_fast(model);
        }
    });
    assert_eq!(reader.fast().delta_best[0], Readout::Live(-2000.0));
    assert_eq!(reader.fast().delta_previous[0], Readout::Live(-4000.0));
}
