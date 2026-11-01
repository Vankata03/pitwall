import { distance as toDistance, speed as toSpeed, type Units } from '../../device/units';
import { BUCKET_METRES, type LapSamples } from './current-lap';

export type Channel = 'throttle' | 'brake' | 'speed';
/** The order series are plotted in, whatever order the setting lists them. */
export const CHANNEL_ORDER: readonly Channel[] = ['throttle', 'brake', 'speed'];

export interface PlotSeries {
  channel: Channel;
  lap: 'current' | 'reference';
  /** Percent for throttle and brake, the device's speed unit for speed; null is a gap. */
  values: (number | null)[];
}

export interface Plot {
  /** Distance in the device's unit, one entry per 5 m bucket from the line. */
  x: number[];
  series: PlotSeries[];
  distanceUnit: string;
  speedUnit: string;
  hasData: boolean;
}

export interface PlotInput {
  channels: readonly Channel[];
  units: Units;
  current: LapSamples;
  reference: LapSamples | null;
}

const lastBucket = (s: LapSamples | null): number =>
  s && s.distance.length > 0 ? Math.round(s.distance[s.distance.length - 1]! / BUCKET_METRES) : -1;

function column(samples: LapSamples, channel: Channel, length: number, units: Units): (number | null)[] {
  const out: (number | null)[] = new Array(length).fill(null);
  const source = samples[channel];
  for (let i = 0; i < samples.distance.length; i++) {
    const v = source[i];
    if (v === null || v === undefined) continue;
    const bucket = Math.round(samples.distance[i]! / BUCKET_METRES);
    if (bucket < 0 || bucket >= length) continue;
    out[bucket] = channel === 'speed' ? toSpeed(v, units).value : v * 100;
  }
  return out;
}

/** Align the live lap and the reference lap on one 5 m axis, ready to hand to the chart. */
export function buildPlot({ channels, units, current, reference }: PlotInput): Plot {
  const length = Math.max(lastBucket(current), lastBucket(reference)) + 1;
  const x = Array.from({ length }, (_, i) => toDistance(i * BUCKET_METRES, units).value);
  const chosen = CHANNEL_ORDER.filter((c) => channels.includes(c));
  const series: PlotSeries[] = [];
  for (const [lap, samples] of [['reference', reference], ['current', current]] as const) {
    if (!samples) continue;
    for (const channel of chosen) series.push({ channel, lap, values: column(samples, channel, length, units) });
  }
  return {
    x,
    series,
    distanceUnit: toDistance(0, units).unit,
    speedUnit: toSpeed(0, units).unit,
    hasData: length > 0,
  };
}
