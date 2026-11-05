//! Ported from `dashboard/src/tiles/traces/current-lap.test.ts`.
use pitwall_core::{
    model::LapSample,
    traces::{BUCKET_METRES, CurrentLap},
};

const NAN: f64 = f64::NAN;

fn close(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-6)
}

#[test]
fn buckets_hold_the_value_at_their_exact_distance_and_samples_between_boundaries_only_shape_it()
{
    let mut lap = CurrentLap::new();
    lap.record(0.0, 100.0, 0.0, 200.0);
    lap.record(5.0, 80.0, 0.0, 210.0);
    lap.record(6.9, 60.0, 10.0, 215.0); // between boundaries: no bucket of its own
    lap.record(10.0, 0.0, 100.0, 150.0);
    let s = lap.samples();
    assert_eq!(s.distance, [0.0, 5.0, 10.0]);
    assert_eq!(s.speed, [Some(200.0), Some(210.0), Some(150.0)]);
    assert_eq!(s.throttle, [Some(1.0), Some(0.8), Some(0.0)]);
    assert_eq!(s.brake, [Some(0.0), Some(0.0), Some(1.0)]);
}

#[test]
fn a_bucket_between_two_samples_is_interpolated_to_its_own_distance() {
    let mut lap = CurrentLap::new();
    lap.record(0.0, 0.0, 100.0, 100.0);
    lap.record(8.0, 80.0, 20.0, 180.0);
    let s = lap.samples();
    assert_eq!(s.distance, [0.0, 5.0]);
    assert!(close(s.throttle[1], 0.5));
    assert!(close(s.brake[1], 0.5));
    assert!(close(s.speed[1], 150.0));
}

#[test]
fn missing_data_leaves_a_gap_and_never_a_zero_and_a_negative_distance_is_dropped() {
    let mut lap = CurrentLap::new();
    lap.record(0.0, 100.0, 0.0, 200.0);
    lap.record(5.0, NAN, NAN, NAN);
    lap.record(-3.0, 50.0, 0.0, 100.0);
    lap.record(10.0, 50.0, 0.0, 100.0);
    lap.record(15.0, 50.0, 0.0, 100.0);
    let s = lap.samples();
    assert_eq!(s.distance, [0.0, 5.0, 10.0, 15.0]);
    // The withheld sample breaks both steps that touch it.
    assert_eq!(s.throttle, [Some(1.0), None, None, Some(0.5)]);
    assert_eq!(s.speed, [Some(200.0), None, None, Some(100.0)]);
}

#[test]
fn a_drop_of_distance_back_to_the_start_line_begins_a_new_lap() {
    let mut lap = CurrentLap::new();
    lap.record(5000.0, 100.0, 0.0, 300.0);
    lap.record(5300.0, 100.0, 0.0, 300.0);
    lap.record(0.0, 90.0, 0.0, 120.0);
    lap.record(10.0, 90.0, 0.0, 120.0);
    let s = lap.samples();
    assert_eq!(s.distance, [0.0, 5.0, 10.0]);
    assert_eq!(s.throttle, [Some(0.9), Some(0.9), Some(0.9)]);
}

#[test]
fn a_small_step_back_as_when_a_car_spins_overwrites_instead_of_resetting() {
    let mut lap = CurrentLap::new();
    lap.record(100.0, 100.0, 0.0, 100.0);
    lap.record(120.0, 100.0, 0.0, 100.0);
    lap.record(110.0, 0.0, 100.0, 50.0);
    assert_eq!(lap.samples().distance, [100.0, 105.0, 110.0, 115.0, 120.0]);
}

#[test]
fn a_fetched_lap_so_far_fills_only_what_the_live_samples_have_not_covered() {
    let mut lap = CurrentLap::new();
    lap.record(20.0, 10.0, 0.0, 90.0);
    let fetched: Vec<LapSample> = (0..5)
        .map(|i| LapSample {
            distance_m: i as f32 * 5.0,
            time_ms: i as f32,
            session_time: 0.0,
            speed_kph: Some(80.0 + 2.0 * i as f32),
            throttle: Some(1.0),
            brake: Some(0.0),
        })
        .collect();
    lap.seed(&fetched);
    let s = lap.samples();
    assert_eq!(s.distance, [0.0, 5.0, 10.0, 15.0, 20.0]);
    assert_eq!(
        s.throttle,
        [Some(1.0), Some(1.0), Some(1.0), Some(1.0), Some(0.1)]
    ); // live bucket 20 kept
    assert_eq!(
        s.speed,
        [Some(80.0), Some(82.0), Some(84.0), Some(86.0), Some(90.0)]
    );
}

#[test]
fn reset_empties_the_lap_and_bumps_the_version_for_the_render_loop() {
    let mut lap = CurrentLap::new();
    lap.record(0.0, 100.0, 0.0, 100.0);
    let before = lap.version();
    lap.reset();
    assert!(lap.samples().distance.is_empty());
    assert!(lap.version() > before);
}

#[test]
fn sparse_samples_at_300_kmh_fill_every_bucket() {
    for steps in [
        &[0.0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3][..],
        &[0.0, 0.05, 0.183, 0.23, 0.41][..],
    ] {
        let mut lap = CurrentLap::new();
        for &t in steps {
            lap.record(t * 300.0 / 3.6, 100.0 - t * 100.0, t * 100.0, 300.0);
        }
        let s = lap.samples();
        let last = (steps.last().unwrap() * 300.0 / 3.6 / 5.0).floor() as usize;
        let expected: Vec<f64> = (0..=last).map(|i| i as f64 * 5.0).collect();
        assert_eq!(s.distance, expected, "{steps:?}");
        assert!(s.throttle.iter().all(Option::is_some), "{steps:?}");
    }
}

#[test]
fn interpolation_keeps_missing_channels_and_gaps_over_100_m_broken() {
    let mut lap = CurrentLap::new();
    lap.record(0.0, 100.0, 0.0, 300.0);
    lap.record(20.0, NAN, 0.0, 300.0);
    lap.record(40.0, 50.0, 0.0, 300.0);
    assert_eq!(lap.samples().throttle[1..8], [None; 7]);
    lap.record(150.0, 100.0, 0.0, 300.0);
    assert!(!lap.samples().distance.contains(&100.0));
}

#[test]
fn every_rate_at_300_kmh_gives_one_continuous_correctly_interpolated_sample_per_bucket() {
    for hz in [10.0, 20.0, 30.0, 60.0] {
        let mut lap = CurrentLap::new();
        let metres_per_sample = 300.0 / 3.6 / hz;
        // Throttle ramps with distance, so any bucket the trace skipped or mis-filled shows.
        let mut n = 0.0;
        while n * metres_per_sample <= 1000.0 {
            let d = n * metres_per_sample;
            lap.record(d, d / 10.0, 100.0 - d / 10.0, 300.0);
            n += 1.0;
        }
        let s = lap.samples();
        let last = ((1000.0 / metres_per_sample).floor() * metres_per_sample / BUCKET_METRES)
            .floor() as usize;
        let expected: Vec<f64> = (0..=last).map(|i| i as f64 * BUCKET_METRES).collect();
        assert_eq!(s.distance, expected, "{hz} Hz");
        for (throttle, distance) in s.throttle.iter().zip(&s.distance) {
            let throttle = throttle.unwrap();
            assert!((0.0..=1.0).contains(&throttle), "{hz} Hz");
            assert!((throttle * 100.0 - distance / 10.0).abs() < 5e-7, "{hz} Hz");
        }
    }
}

#[test]
fn a_gap_of_exactly_100_m_is_joined_and_one_past_it_is_not() {
    let mut joined = CurrentLap::new();
    joined.record(0.0, 0.0, 0.0, 300.0);
    joined.record(100.0, 100.0, 0.0, 300.0);
    assert!(joined.samples().throttle.iter().all(Option::is_some));
    let mut broken = CurrentLap::new();
    broken.record(0.0, 0.0, 0.0, 300.0);
    broken.record(100.5, 100.0, 0.0, 300.0);
    assert_eq!(broken.samples().distance, [0.0]);
}

#[test]
fn a_withheld_channel_breaks_only_itself_and_recovers_on_the_next_pair_of_real_samples() {
    let mut lap = CurrentLap::new();
    lap.record(0.0, 0.0, 100.0, 300.0);
    lap.record(20.0, NAN, 100.0, 300.0);
    lap.record(40.0, 100.0, 100.0, 300.0);
    lap.record(60.0, 100.0, 100.0, 300.0);
    let s = lap.samples();
    assert!(s.brake.iter().all(|v| *v == Some(1.0)));
    assert_eq!(s.throttle[0], Some(0.0));
    assert_eq!(s.throttle[1..9], [None; 8]);
    assert_eq!(s.throttle[9..], [Some(1.0); 4]);
}

#[test]
fn joining_mid_lap_is_detected_from_the_first_bucket() {
    let mut lap = CurrentLap::new();
    lap.record(5.0, 0.0, 0.0, 100.0);
    assert!(!lap.joined_mid_lap());
    let mut late = CurrentLap::new();
    late.record(800.0, 0.0, 0.0, 100.0);
    assert!(late.joined_mid_lap());
}
