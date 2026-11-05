//! Ported from `dashboard/src/tiles/traces/smooth.test.ts` and the smoothing case of
//! `dashboard/src/overlay/throttle-brake.test.tsx`.
use pitwall_core::traces::{smooth_line, smooth_values};

#[test]
fn averages_within_the_radius_keeps_both_ends_exact_and_never_reaches_across_a_gap() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    let values = [
        Some(0.0),
        Some(0.0),
        Some(30.0),
        Some(30.0),
        Some(60.0),
        None,
        Some(90.0),
        Some(0.0),
    ];
    assert_eq!(
        smooth_values(&x, &values, 1.0),
        [
            Some(0.0),
            Some(10.0),
            Some(20.0),
            Some(40.0),
            Some(60.0),
            None,
            Some(90.0),
            Some(0.0)
        ]
    );
}

#[test]
fn a_zero_radius_changes_nothing() {
    let values = [Some(0.0), Some(50.0), Some(0.0)];
    assert_eq!(smooth_values(&[0.0, 1.0, 2.0], &values, 0.0), values);
}

#[test]
fn line_smoothing_averages_neighbours_within_the_radius_and_keeps_the_ends() {
    let stairs = vec![
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 30.0),
        (3.0, 30.0),
        (4.0, 60.0),
    ];
    assert_eq!(
        smooth_line(stairs.clone(), 1.0),
        [
            (0.0, 0.0),
            (1.0, 10.0),
            (2.0, 20.0),
            (3.0, 40.0),
            (4.0, 60.0)
        ]
    );
    assert_eq!(smooth_line(stairs.clone(), 0.0), stairs);
}
