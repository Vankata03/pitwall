import { speed, type Units } from '../../device/units';
import type { Trace } from '../../generated/wire/Trace';
import type { CarRings } from '../../runtime/stores';
import { RingBuffer } from '../../runtime/ring';
import { CHANNEL_ORDER, type Channel, type Plot, type PlotSeries } from './plot';

export const INPUT_WINDOW_SECONDS = 30;
type InputRings = Pick<CarRings, 'sessionTime' | 'lapDistance' | 'throttle' | 'brake' | 'speed'>;
const fields = ['sessionTime', 'lapDistance', 'throttle', 'brake', 'speed'] as const;

/** The inputs tile owns its longer history; paused publications replace the current instant. */
export class InputHistory {
  readonly rings: InputRings = Object.fromEntries(fields.map(field => [field, new RingBuffer(1801)])) as unknown as InputRings;
  private identity = '';
  private consumed = 0;

  update(source: InputRings | null, identity: string): void {
    if (identity !== this.identity) {
      for (const field of fields) this.rings[field].clear();
      this.identity = identity;
      this.consumed = source ? source.sessionTime.version - source.sessionTime.length : 0;
    }
    if (!source) return;
    const added = Math.min(source.sessionTime.length, Math.max(0, source.sessionTime.version - this.consumed));
    for (let i = source.sessionTime.length - added; i < source.sessionTime.length; i++) {
      const time = source.sessionTime.at(i);
      if (time < this.rings.sessionTime.last()) for (const field of fields) this.rings[field].clear();
      const duplicate = time === this.rings.sessionTime.last();
      for (const field of fields) {
        if (duplicate) this.rings[field].replaceLast(source[field].at(i));
        else this.rings[field].push(source[field].at(i));
      }
    }
    this.consumed = source.sessionTime.version;
  }
}

/** Reference inputs at the same circuit position as the live sample. Never extrapolate. */
function referenceValue(trace: Trace, distance: number, channel: Channel): number | null {
  if (!Number.isFinite(distance)) return null;
  let lo = 0, hi = trace.distance.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (trace.distance[mid]! < distance) lo = mid + 1;
    else hi = mid;
  }
  const right = trace[channel][lo];
  if (trace.distance[lo] === distance) return right ?? null;
  const left = trace[channel][lo - 1];
  const start = trace.distance[lo - 1], end = trace.distance[lo];
  if (left == null || right == null || start === undefined || end === undefined || end - start > 100) return null;
  return left + (right - left) * (distance - start) / (end - start);
}

/** Recent telemetry in session time: the axis never grows, and crossing the line preserves inputs. */
export function buildWindowPlot(rings: InputRings | null, reference: Trace | null, channels: readonly Channel[], units: Units): Plot {
  const chosen = CHANNEL_ORDER.filter(c => channels.includes(c));
  const series: PlotSeries[] = [];
  if (reference) for (const channel of chosen) series.push({ channel, lap: 'reference', values: [] });
  for (const channel of chosen) series.push({ channel, lap: 'current', values: [] });
  const plot: Plot = { x: [], series, distanceUnit: 's', windowSeconds: INPUT_WINDOW_SECONDS, speedUnit: speed(0, units).unit, hasData: false };
  if (!rings?.sessionTime.length) return plot;
  const times = rings.sessionTime;
  const now = times.last();
  let first = times.length - 1;
  while (first > 0 && times.at(first - 1) >= now - INPUT_WINDOW_SECONDS && times.at(first - 1) <= times.at(first)) first--;
  let previous = -Infinity;
  for (let i = first; i < times.length; i++) {
    const time = times.at(i);
    if (!Number.isFinite(time) || time <= previous) continue;
    if (Number.isFinite(previous) && time - previous > 0.5) {
      plot.x.push((previous + time) / 2 - now);
      for (const s of series) s.values.push(null);
    }
    plot.x.push(time - now);
    for (const s of series) {
      const raw = s.lap === 'current' ? rings[s.channel].at(i) : referenceValue(reference!, rings.lapDistance.at(i), s.channel);
      const value = raw === null || !Number.isFinite(raw) ? null
        : s.channel === 'speed' ? speed(raw, units).value
        : s.lap === 'reference' ? raw * 100 : raw;
      s.values.push(value);
      if (s.lap === 'current' && value !== null) plot.hasData = true;
    }
    previous = time;
  }
  return plot;
}
