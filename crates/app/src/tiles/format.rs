//! Number formats shared by tiles (from `dashboard/src/tiles/frame-kit.tsx`).

/// The real minus sign, so deltas read `+` or `−` as the spec asks.
pub const MINUS: char = '\u{2212}';

/// Signed with `digits` decimals; a value that rounds to zero reads `+`.
pub fn signed(value: f64, digits: usize) -> String {
    let text = format!("{:.digits$}", value.abs());
    let zero = text.chars().all(|c| c == '0' || c == '.');
    let sign = if value < 0.0 && !zero { MINUS } else { '+' };
    format!("{sign}{text}")
}

/// JavaScript's `toFixed`: `digits` decimals, a tie rounding away from zero (Rust's
/// formatter rounds a tie to even, so `18.25` would read `18.2`).
pub fn fixed(value: f64, digits: usize) -> String {
    let scale = 10f64.powi(digits as i32);
    let rounded = (value * scale).round() / scale;
    format!("{:.digits$}", if rounded == 0.0 { 0.0 } else { rounded })
}

/// Rounded integer with en-US thousands separators.
pub fn grouped(value: f64) -> String {
    let rounded = value.round() as i64;
    let digits = rounded.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if rounded < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Lap and sector clock: `m:ss.mmm` for a lap or from a minute up, else `s.mmm`.
pub fn clock(ms: u32, lap: bool) -> String {
    let seconds = (ms / 1000) % 60;
    let millis = ms % 1000;
    if lap || ms >= 60_000 {
        format!("{}:{seconds:02}.{millis:03}", ms / 60_000)
    } else {
        format!("{seconds}.{millis:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_values_use_the_real_minus_and_zero_reads_plus() {
        assert_eq!(signed(-0.4321, 3), "\u{2212}0.432");
        assert_eq!(signed(1.5, 3), "+1.500");
        assert_eq!(signed(-0.0004, 3), "+0.000");
        assert_eq!(signed(0.0, 3), "+0.000");
    }

    #[test]
    fn fixed_rounds_ties_away_from_zero_like_javascript() {
        assert_eq!(fixed(18.25, 1), "18.3");
        assert_eq!(fixed(1.2, 2), "1.20");
        assert_eq!(fixed(-0.001, 2), "0.00");
        assert_eq!(fixed(21.8 * 0.0689475729, 2), "1.50");
    }

    #[test]
    fn grouped_rounds_and_separates_thousands() {
        assert_eq!(grouped(11800.4), "11,800");
        assert_eq!(grouped(999.0), "999");
        assert_eq!(grouped(1_234_567.0), "1,234,567");
        assert_eq!(grouped(0.0), "0");
    }

    #[test]
    fn clock_reads_m_ss_mmm_for_laps_and_long_sectors() {
        assert_eq!(clock(83_456, true), "1:23.456");
        assert_eq!(clock(5_007, true), "0:05.007");
        assert_eq!(clock(28_123, false), "28.123");
        assert_eq!(clock(61_000, false), "1:01.000");
    }
}
