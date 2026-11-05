import type { Trace } from '../generated/wire/Trace';
import { PEDAL_SMOOTH_SECONDS, smoothValues } from '../tiles/traces/smooth';
import { referenceValue, type InputHistory } from '../tiles/traces/window-plot';

/** Seconds of input history the overlay trace shows. */
export const OVERLAY_WINDOW_SECONDS = 5;
/** A pause in samples longer than this breaks the line instead of joining across it. */
const GAP_SECONDS = 0.5;

/** One pedal's trace as canvas polylines: x in [0, width], y in [0, height] with 100% at the top. */
export type Polyline = Array<[number, number]>;

export interface InputTrace {
  throttle: Polyline[];
  brake: Polyline[];
  /** The reference lap's inputs at the same circuit positions; empty without a reference. */
  reference: { throttle: Polyline[]; brake: Polyline[] };
}

type Channel = 'throttle' | 'brake';
const CHANNELS: readonly Channel[] = ['throttle', 'brake'];

/**
 * Turn the newest `OVERLAY_WINDOW_SECONDS` of inputs into polylines for a `width` × `height`
 * box. The newest sample sits on the right edge. Session-time gaps split the line.
 * With a `reference` lap, each live sample also gets that lap's input at the same lap distance,
 * so the two laps overlap corner for corner. Where the reference has no data the line breaks.
 */
export function inputTrace(rings: InputHistory['rings'], width: number, height: number, reference: Trace | null = null): InputTrace {
  const out: InputTrace = { throttle: [], brake: [], reference: { throttle: [], brake: [] } };
  const times = rings.sessionTime;
  if (!times.length || width <= 0 || height <= 0) return out;
  const now = times.last();
  let first = times.length - 1;
  while (first > 0 && times.at(first - 1) >= now - OVERLAY_WINDOW_SECONDS) first--;
  let previous = -Infinity;
  let current: Record<Channel, Polyline> | null = null;
  let ref: Record<Channel, Polyline | null> = { throttle: null, brake: null };
  const y = (percent: number) => height * (1 - Math.min(100, Math.max(0, percent)) / 100);
  for (let i = first; i < times.length; i++) {
    const time = times.at(i);
    if (!Number.isFinite(time) || time <= previous) continue;
    if (!current || time - previous > GAP_SECONDS) {
      current = { throttle: [], brake: [] };
      out.throttle.push(current.throttle);
      out.brake.push(current.brake);
      ref = { throttle: null, brake: null };
    }
    const x = width * (1 + (time - now) / OVERLAY_WINDOW_SECONDS);
    for (const channel of CHANNELS) {
      const value = rings[channel].at(i);
      if (Number.isFinite(value)) current[channel].push([x, y(value)]);
      if (!reference) continue;
      const past = referenceValue(reference, rings.lapDistance.at(i), channel);
      if (past === null || !Number.isFinite(past)) {
        ref[channel] = null;
        continue;
      }
      if (!ref[channel]) out.reference[channel].push(ref[channel] = []);
      // Lap traces store inputs as fractions; live inputs are percentages.
      ref[channel]!.push([x, y(past * 100)]);
    }
    previous = time;
  }
  const radius = width * PEDAL_SMOOTH_SECONDS / OVERLAY_WINDOW_SECONDS;
  for (const lines of [out.throttle, out.brake, out.reference.throttle, out.reference.brake]) {
    for (let i = 0; i < lines.length; i++) lines[i] = smoothLine(lines[i]!, radius);
  }
  return out;
}

/** `smoothValues` for a polyline: smooths `y` against `x`, so both ends stay exact. */
export function smoothLine(line: Polyline, radius: number): Polyline {
  if (radius <= 0 || line.length < 3) return line;
  const y = smoothValues(line.map(([x]) => x), line.map(([, v]) => v), radius);
  return line.map(([x], i) => [x, y[i]!]);
}
