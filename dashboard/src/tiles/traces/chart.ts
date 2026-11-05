import type uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import type { Channel, Plot } from './plot';
import { PEDAL_SMOOTH_SECONDS, smoothValues } from './smooth';

/**
 * The one place uPlot is touched. Canvas 2D, no React: the tile hands it a `Plot` each time
 * the data changed and it redraws in place.
 */
export interface TraceChart {
  draw(plot: Plot, size: { width: number; height: number }): void;
  destroy(): void;
}

const FALLBACK = { energy: '#56D9CA', brake: '#F55C72', text: '#F3F5F6', muted: '#9AA7AD', line: '#343d42' };

function palette(host: HTMLElement) {
  const css = getComputedStyle(host);
  const read = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    throttle: read('--pw-energy', FALLBACK.energy),
    brake: read('--pw-brake', FALLBACK.brake),
    speed: read('--pw-text', FALLBACK.text),
    muted: read('--pw-text-muted', FALLBACK.muted),
    line: read('--pw-line', FALLBACK.line),
    font: read('--pw-font', 'system-ui, sans-serif'),
  };
}

const SCALE: Record<Channel, 'pct' | 'spd'> = { throttle: 'pct', brake: 'pct', speed: 'spd' };
/** Chart rows: pedal lines smoothed when the x axis is session time, the rest as they are. */
const rowsOf = (plot: Plot) => [plot.x, ...plot.series.map((s) =>
  plot.distanceUnit === 's' && s.channel !== 'speed' ? smoothValues(plot.x, s.values, PEDAL_SMOOTH_SECONDS) : s.values,
)] as uPlot.AlignedData;

const shapeOf = (plot: Plot) =>
  [plot.distanceUnit, plot.windowSeconds, plot.speedUnit, ...plot.series.map((s) => `${s.lap}:${s.channel}`)].join('|');

function loadedChart(host: HTMLElement, PlotConstructor: typeof uPlot): TraceChart {
  let chart: uPlot | null = null;
  let shape = '';

  function build(plot: Plot, size: { width: number; height: number }): uPlot {
    const colour = palette(host);
    const font = `11px ${colour.font}`;
    const axis = { stroke: colour.muted, font, grid: { stroke: colour.line, width: 1 }, ticks: { stroke: colour.line, width: 1 } };
    const hasSpeed = plot.series.some((s) => s.channel === 'speed');
    const hasPct = plot.series.some((s) => SCALE[s.channel] === 'pct');
    // Monotone splines round off 60 Hz sample steps without overshooting the data.
    const smooth = PlotConstructor.paths.spline?.();
    const options: uPlot.Options = {
      ...size,
      legend: { show: false },
      cursor: { show: false },
      select: { show: false, left: 0, top: 0, width: 0, height: 0 },
      scales: {
        x: { time: false, ...(plot.windowSeconds === undefined ? {} : { range: () => [-plot.windowSeconds!, 0] as [number, number] }) },
        pct: { range: [0, 100] },
        spd: { range: (_u, _min, max) => [0, Math.max(50, Math.ceil(max / 50) * 50)] },
      },
      axes: [
        { ...axis, scale: 'x', size: 24 },
        ...(hasPct ? [{ ...axis, scale: 'pct', size: 44, values: (_u: uPlot, v: number[]) => v.map((n) => `${n}%`) }] : []),
        ...(hasSpeed
          ? [{ ...axis, scale: 'spd', side: 1, size: 52, grid: { show: !hasPct }, values: (_u: uPlot, v: number[]) => v.map((n) => `${n}`) }]
          : []),
      ],
      series: [
        {},
        ...plot.series.map((s) => ({
          scale: SCALE[s.channel],
          stroke: colour[s.channel],
          // Bright, heavier current traces sit over subdued solid reference traces.
          width: s.lap === 'current' ? 3.5 : 1.5,
          alpha: s.lap === 'current' ? 1 : 0.4,
          spanGaps: false,
          points: { show: false },
          ...(smooth ? { paths: smooth } : {}),
        })),
      ],
    };
    return new PlotConstructor(options, rowsOf(plot), host);
  }

  return {
    draw(plot, size) {
      const next = shapeOf(plot);
      if (chart && next !== shape) {
        chart.destroy();
        chart = null;
      }
      shape = next;
      if (!chart) {
        chart = build(plot, size);
        return;
      }
      if (chart.width !== size.width || chart.height !== size.height) chart.setSize(size);
      chart.setData(rowsOf(plot));
    },
    destroy() {
      chart?.destroy();
      chart = null;
    },
  };
}


/** Load the browser-only renderer when a tile draws, so catalogue discovery stays inert. */
export function createTraceChart(host: HTMLElement): TraceChart {
  let active = true;
  let chart: TraceChart | null = null;
  let loading = false;
  let latest: { plot: Plot; size: { width: number; height: number } } | null = null;
  return {
    draw(plot, size) {
      latest = { plot, size };
      if (chart) chart.draw(plot, size);
      else if (!loading) {
        loading = true;
        void import('uplot').then(({ default: uPlot }) => {
          if (!active) return;
          chart = loadedChart(host, uPlot);
          if (latest) chart.draw(latest.plot, latest.size);
        });
      }
    },
    destroy() {
      active = false;
      latest = null;
      chart?.destroy();
    },
  };
}
