//! Ported from `dashboard/src/tiles/traces/window-plot.test.ts`.
use pitwall_core::{
    model::LapSample,
    traces::{
        Channel, InputHistory, InputSample, Lap, WindowPlot, build_window_plot, reference_value,
    },
};

struct Samples(InputHistory);

impl Samples {
    fn new() -> Self {
        Self(InputHistory::new())
    }
    fn add(&mut self, time: f64, distance: f64, throttle: f64, brake: f64) {
        self.0.record(
            (1, 0),
            InputSample {
                session_time: time,
                lap_distance: distance,
                throttle,
                brake,
                speed: 200.0,
            },
        );
    }
    fn add_at(&mut self, time: f64) {
        self.add(time, 0.0, 70.0, 0.0);
    }
    fn plot(&self, reference: Option<&[LapSample]>) -> WindowPlot {
        build_window_plot(
            &self.0,
            reference,
            &[Channel::Throttle, Channel::Brake],
            |kmh| kmh,
        )
    }
}

fn values(plot: &WindowPlot, lap: Lap, channel: Channel) -> Vec<Option<f64>> {
    plot.series
        .iter()
        .find(|s| s.lap == lap && s.channel == channel)
        .unwrap()
        .values
        .clone()
}

fn sample(distance: f32, speed: f32, throttle: f32, brake: f32) -> LapSample {
    LapSample {
        distance_m: distance,
        time_ms: 0.0,
        session_time: 0.0,
        speed_kph: Some(speed),
        throttle: Some(throttle),
        brake: Some(brake),
    }
}

#[test]
fn keeps_exactly_thirty_seconds_at_60_hz_including_stationary_pedal_changes() {
    let mut s = Samples::new();
    for i in 0..=2400 {
        s.add(
            f64::from(i) / 60.0,
            0.0,
            if i % 2 == 1 { 20.0 } else { 70.0 },
            0.0,
        );
    }
    let p = s.plot(None);
    assert_eq!(p.window_seconds, 30.0);
    assert_eq!(p.x.len(), 1801);
    assert_eq!(p.x[0], -30.0);
    assert_eq!(*p.x.last().unwrap(), 0.0);
    let throttle = values(&p, Lap::Current, Channel::Throttle);
    assert_eq!(throttle[throttle.len() - 2..], [Some(20.0), Some(70.0)]);
}

#[test]
fn a_new_stream_keeps_the_full_axis_even_before_thirty_seconds_have_arrived() {
    let mut s = Samples::new();
    s.add_at(120.0);
    s.add_at(120.1);
    let p = s.plot(None);
    assert_eq!(p.window_seconds, 30.0);
    assert!((p.x[0] + 0.1).abs() < 1e-9);
    assert_eq!(*p.x.last().unwrap(), 0.0);
}

#[test]
fn paused_replay_publications_do_not_evict_the_thirty_seconds_already_received() {
    let mut s = Samples::new();
    for i in 0..=1800 {
        s.add_at(f64::from(i) / 60.0);
    }
    for _ in 0..2000 {
        s.add_at(30.0);
    }
    s.add(30.0, 0.0, 40.0, 10.0);
    let p = s.plot(None);
    assert_eq!(p.x.len(), 1801);
    assert_eq!(p.x[0], -30.0);
    assert_eq!(
        values(&p, Lap::Current, Channel::Throttle).last(),
        Some(&Some(40.0))
    );
    assert_eq!(
        values(&p, Lap::Current, Channel::Brake).last(),
        Some(&Some(10.0))
    );
}

#[test]
fn breaks_lines_across_missing_telemetry_and_discards_history_after_a_time_reversal() {
    let mut s = Samples::new();
    s.add_at(10.0);
    s.add_at(12.0);
    assert_eq!(
        values(&s.plot(None), Lap::Current, Channel::Throttle),
        [Some(70.0), None, Some(70.0)]
    );
    s.add(5.0, 0.0, 30.0, 0.0);
    let p = s.plot(None);
    assert_eq!(p.x, [0.0]);
    assert_eq!(values(&p, Lap::Current, Channel::Throttle), [Some(30.0)]);
}

#[test]
fn aligns_the_dim_reference_to_circuit_position_within_the_time_window() {
    let mut s = Samples::new();
    s.add(10.0, 2.5, 70.0, 0.0);
    s.add(10.1, 5.0, 70.0, 0.0);
    s.add(10.2, 100.0, 70.0, 0.0);
    let reference = [sample(0.0, 100.0, 1.0, 0.0), sample(5.0, 200.0, 0.0, 1.0)];
    let p = s.plot(Some(&reference));
    assert_eq!(
        values(&p, Lap::Reference, Channel::Throttle),
        [Some(50.0), Some(0.0), None]
    );
    assert_eq!(
        values(&p, Lap::Reference, Channel::Brake),
        [Some(50.0), Some(100.0), None]
    );
}

#[test]
fn a_new_identity_starts_a_new_history() {
    let mut history = InputHistory::new();
    let at = |time| InputSample {
        session_time: time,
        lap_distance: 0.0,
        throttle: 10.0,
        brake: 0.0,
        speed: 0.0,
    };
    history.record((1, 0), at(1.0));
    history.record((1, 0), at(2.0));
    history.record((2, 0), at(3.0));
    assert_eq!(history.len(), 1);
    history.record((2, 5), at(4.0));
    assert_eq!(history.len(), 1);
}

#[test]
fn reference_values_interpolate_by_distance_and_never_extrapolate() {
    let reference = [
        sample(100.0, 0.0, 0.2, 0.0),
        sample(110.0, 0.0, 0.4, 1.0),
        sample(250.0, 0.0, 1.0, 1.0),
    ];
    let at = |d| reference_value(&reference, d, Channel::Throttle);
    assert_eq!(at(110.0), Some(0.4000000059604645));
    assert!((at(105.0).unwrap() - 0.3).abs() < 1e-6);
    assert_eq!(at(99.0), None); // before the first sample
    assert_eq!(at(260.0), None); // past the last sample
    assert_eq!(at(200.0), None); // neighbours more than 100 m apart
    assert_eq!(at(f64::NAN), None);
}
