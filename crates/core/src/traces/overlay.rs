use crate::model::LapSample;

use super::{
    Channel, GAP_SECONDS, InputHistory, PEDAL_SMOOTH_SECONDS, Polyline, reference_value,
    smooth_line,
};

/// Seconds of input history the overlay trace shows.
pub const OVERLAY_WINDOW_SECONDS: f64 = 5.0;

/// The overlay trace as polylines, one per unbroken stretch of samples.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputTrace {
    pub throttle: Vec<Polyline>,
    pub brake: Vec<Polyline>,
    /// The reference lap's inputs at the same circuit positions; empty without a reference.
    pub reference: ReferenceTrace,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReferenceTrace {
    pub throttle: Vec<Polyline>,
    pub brake: Vec<Polyline>,
}

const CHANNELS: [Channel; 2] = [Channel::Throttle, Channel::Brake];

/// Turn the newest [`OVERLAY_WINDOW_SECONDS`] of inputs into polylines for a `width` ×
/// `height` box. The newest sample sits on the right edge, and session-time gaps over half a
/// second split the line. A withheld pedal value is skipped without splitting it. With a
/// `reference` lap, each live sample also gets that lap's input at the same lap distance, so
/// the two laps overlap corner for corner; where the reference has no data its line breaks.
/// Every line is smoothed over ±60 ms, which keeps both ends exact.
pub fn input_trace(
    history: &InputHistory,
    width: f64,
    height: f64,
    reference: Option<&[LapSample]>,
) -> InputTrace {
    let mut out = InputTrace::default();
    let samples = history.samples();
    let Some(now) = samples.back().map(|s| s.session_time) else {
        return out;
    };
    if width.is_nan() || height.is_nan() || width <= 0.0 || height <= 0.0 {
        return out;
    }
    let mut first = samples.len() - 1;
    while first > 0 && samples[first - 1].session_time >= now - OVERLAY_WINDOW_SECONDS {
        first -= 1;
    }
    let y = |percent: f64| height * (1.0 - percent.clamp(0.0, 100.0) / 100.0);
    let mut previous = f64::NEG_INFINITY;
    let mut started = false;
    // Whether each channel's reference line is open, so the next point extends it.
    let mut reference_open = [false; 2];
    for sample in samples.range(first..) {
        let time = sample.session_time;
        if !time.is_finite() || time <= previous {
            continue;
        }
        if !started || time - previous > GAP_SECONDS {
            out.throttle.push(vec![]);
            out.brake.push(vec![]);
            reference_open = [false; 2];
            started = true;
        }
        let x = width * (1.0 + (time - now) / OVERLAY_WINDOW_SECONDS);
        for (index, channel) in CHANNELS.into_iter().enumerate() {
            let (lines, reference_lines) = match channel {
                Channel::Throttle => (&mut out.throttle, &mut out.reference.throttle),
                _ => (&mut out.brake, &mut out.reference.brake),
            };
            let value = sample.value(channel);
            if value.is_finite() {
                lines.last_mut().unwrap().push((x, y(value)));
            }
            let Some(reference) = reference else { continue };
            match reference_value(reference, sample.lap_distance, channel) {
                Some(past) if past.is_finite() => {
                    if !reference_open[index] {
                        reference_lines.push(vec![]);
                        reference_open[index] = true;
                    }
                    // Lap traces store inputs as fractions; live inputs are percentages.
                    reference_lines.last_mut().unwrap().push((x, y(past * 100.0)));
                }
                _ => reference_open[index] = false,
            }
        }
        previous = time;
    }
    let radius = width * PEDAL_SMOOTH_SECONDS / OVERLAY_WINDOW_SECONDS;
    for lines in [
        &mut out.throttle,
        &mut out.brake,
        &mut out.reference.throttle,
        &mut out.reference.brake,
    ] {
        for line in lines.iter_mut() {
            *line = smooth_line(std::mem::take(line), radius);
        }
    }
    out
}
