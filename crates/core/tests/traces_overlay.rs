//! Ported from the trace cases of `dashboard/src/overlay/throttle-brake.test.tsx`.
use pitwall_core::{
    model::LapSample,
    traces::{InputHistory, InputSample, Polyline, input_trace},
};

fn history(samples: &[(f64, f64, f64, f64)]) -> InputHistory {
    let mut h = InputHistory::new();
    for &(time, distance, throttle, brake) in samples {
        h.record(
            (1, 0),
            InputSample {
                session_time: time,
                lap_distance: distance,
                throttle,
                brake,
                speed: 0.0,
            },
        );
    }
    h
}

fn rounded(lines: &[Polyline]) -> Vec<Vec<(f64, f64)>> {
    lines
        .iter()
        .map(|line| line.iter().map(|&(x, y)| (x.round(), y.round())).collect())
        .collect()
}

#[test]
fn trace_keeps_the_last_5_s_with_the_newest_sample_on_the_right_edge() {
    let h = history(&[
        (0.0, 0.0, 100.0, 0.0),
        (10.0, 0.0, 100.0, 0.0),
        (10.4, 0.0, 50.0, 0.0),
        (10.8, 0.0, 0.0, 100.0),
    ]);
    let trace = input_trace(&h, 100.0, 50.0, None);
    assert_eq!(
        rounded(&trace.throttle),
        [[(84.0, 0.0), (92.0, 25.0), (100.0, 50.0)]]
    );
    assert_eq!(
        rounded(&trace.brake),
        [[(84.0, 50.0), (92.0, 50.0), (100.0, 0.0)]]
    );
}

#[test]
fn trace_splits_at_a_pause_in_samples() {
    let h = history(&[
        (1.0, 0.0, 10.0, 0.0),
        (1.1, 0.0, 20.0, 0.0),
        (3.0, 0.0, 30.0, 0.0),
        (3.1, 0.0, 40.0, 0.0),
    ]);
    assert_eq!(input_trace(&h, 100.0, 100.0, None).throttle.len(), 2);
}

#[test]
fn a_withheld_pedal_value_is_skipped_without_splitting_the_line() {
    let h = history(&[
        (1.0, 0.0, 10.0, 0.0),
        (1.1, 0.0, f64::NAN, 0.0),
        (1.2, 0.0, 30.0, 0.0),
    ]);
    let trace = input_trace(&h, 100.0, 100.0, None);
    assert_eq!(trace.throttle.len(), 1);
    assert_eq!(trace.throttle[0].len(), 2);
    assert_eq!(trace.brake[0].len(), 3);
}

#[test]
fn trace_overlaps_the_reference_lap_at_the_same_lap_distance() {
    let h = history(&[
        (10.0, 100.0, 100.0, 0.0),
        (10.4, 110.0, 50.0, 0.0),
        (10.8, 120.0, 0.0, 0.0),
    ]);
    // Reference covers 100 to 110 m only: throttle 20% then 40%, brake on at 110 m.
    let sample = |distance, throttle, brake| LapSample {
        distance_m: distance,
        time_ms: 0.0,
        session_time: 0.0,
        speed_kph: Some(0.0),
        throttle: Some(throttle),
        brake: Some(brake),
    };
    let reference = [sample(100.0, 0.2, 0.0), sample(110.0, 0.4, 1.0)];
    let trace = input_trace(&h, 100.0, 100.0, Some(&reference));
    assert_eq!(
        rounded(&trace.reference.throttle),
        [[(84.0, 80.0), (92.0, 60.0)]]
    );
    assert_eq!(
        rounded(&trace.reference.brake),
        [[(84.0, 100.0), (92.0, 0.0)]]
    );
    assert!(
        input_trace(&h, 100.0, 100.0, None)
            .reference
            .throttle
            .is_empty()
    );
}

#[test]
fn an_empty_history_or_box_draws_nothing() {
    let h = history(&[(1.0, 0.0, 10.0, 0.0), (1.1, 0.0, 20.0, 0.0)]);
    assert!(
        input_trace(&InputHistory::new(), 100.0, 100.0, None)
            .throttle
            .is_empty()
    );
    assert!(input_trace(&h, 0.0, 100.0, None).throttle.is_empty());
}
