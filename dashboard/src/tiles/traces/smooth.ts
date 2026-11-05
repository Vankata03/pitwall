/**
 * Half-width, in seconds, of the moving average on pedal lines. Inputs often hold one value for
 * a few 60 Hz samples, which draws as stairs; ±60 ms rounds those off and keeps real stabs.
 * This is presentation only: readouts and stored samples stay exact.
 */
export const PEDAL_SMOOTH_SECONDS = 0.06;

/**
 * Centred moving average of `values` over the points within `radius` of each `x`, with `x`
 * ascending. Near either end of a run the window narrows to stay symmetric, so the newest and
 * oldest values are exact and the line never lags. A null is a gap: it stays null and the
 * average never reaches across it.
 */
export function smoothValues(x: ArrayLike<number>, values: readonly (number | null)[], radius: number): (number | null)[] {
  const out = values.slice();
  if (!(radius > 0)) return out;
  let start = 0;
  while (start < values.length) {
    if (values[start] === null) { start++; continue; }
    let end = start;
    while (end < values.length && values[end] !== null) end++;
    for (let i = start; i < end; i++) {
      const r = Math.min(radius, x[i]! - x[start]!, x[end - 1]! - x[i]!);
      let sum = values[i]!, count = 1;
      for (let j = i - 1; j >= start && x[i]! - x[j]! <= r; j--) { sum += values[j]!; count++; }
      for (let j = i + 1; j < end && x[j]! - x[i]! <= r; j++) { sum += values[j]!; count++; }
      out[i] = sum / count;
    }
    start = end;
  }
  return out;
}
