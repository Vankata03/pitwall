/// Half-width, in seconds, of the moving average on pedal lines. Inputs often hold one value
/// for a few 60 Hz samples, which draws as stairs; ±60 ms rounds those off and keeps real
/// stabs. This is presentation only: readouts and stored samples stay exact.
pub const PEDAL_SMOOTH_SECONDS: f64 = 0.06;

/// Centred moving average of `values` over the points within `radius` of each `x`, with `x`
/// ascending. Near either end of a run the window narrows to stay symmetric, so the newest
/// and oldest values are exact and the line never lags. A `None` is a gap: it stays `None`
/// and the average never reaches across it.
pub fn smooth_values(x: &[f64], values: &[Option<f64>], radius: f64) -> Vec<Option<f64>> {
    let mut out = values.to_vec();
    if radius.is_nan() || radius <= 0.0 {
        return out;
    }
    let mut start = 0;
    while start < values.len() {
        if values[start].is_none() {
            start += 1;
            continue;
        }
        let end = values[start..]
            .iter()
            .position(Option::is_none)
            .map_or(values.len(), |n| start + n);
        let run: Vec<f64> = values[start..end].iter().flatten().copied().collect();
        out[start..end].copy_from_slice(&smooth_run(&x[start..end], &run, radius));
        start = end;
    }
    out
}

/// [`smooth_values`] for a polyline: smooths `y` against `x`, so both ends stay exact.
pub fn smooth_line(line: Vec<(f64, f64)>, radius: f64) -> Vec<(f64, f64)> {
    if radius.is_nan() || radius <= 0.0 || line.len() < 3 {
        return line;
    }
    let x: Vec<f64> = line.iter().map(|p| p.0).collect();
    let y: Vec<f64> = line.iter().map(|p| p.1).collect();
    let smoothed = smooth_run(&x, &y, radius);
    x.into_iter().zip(smoothed.into_iter().flatten()).collect()
}

/// One gap-free run. Every output is `Some`; the option only matches `smooth_values`.
fn smooth_run(x: &[f64], values: &[f64], radius: f64) -> Vec<Option<f64>> {
    let last = values.len() - 1;
    (0..values.len())
        .map(|i| {
            let r = radius.min(x[i] - x[0]).min(x[last] - x[i]);
            let (mut sum, mut count) = (values[i], 1.0);
            for j in (0..i).rev().take_while(|&j| x[i] - x[j] <= r) {
                sum += values[j];
                count += 1.0;
            }
            for j in (i + 1..=last).take_while(|&j| x[j] - x[i] <= r) {
                sum += values[j];
                count += 1.0;
            }
            Some(sum / count)
        })
        .collect()
}
