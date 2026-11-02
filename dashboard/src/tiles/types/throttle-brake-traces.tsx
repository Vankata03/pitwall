import { useEffect, useRef, useSyncExternalStore } from 'react';
import type { ReadoutState } from '../../runtime/wire';
import { useRuntime, useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState, writeReadout } from '../../runtime/readout';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import '../frame-tiles.css';
import { TileTitle } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';
import { buildWindowPlot, InputHistory } from '../traces/window-plot';
import { lapTracesFor } from '../traces/lap-traces';
import { CHANNEL_ORDER, type Channel } from '../traces/plot';
import { createTraceChart, type TraceChart } from '../traces/chart';

/** Space the tile's padding, title and meters take from the chart, in CSS pixels. */
const CHART_INSET = { width: 138, height: 45 };

/** One pedal: its value right now, beside the recent input trace. */
function Meter({ name, label, fill, value }: { name: string; label: string; fill: React.Ref<HTMLElement>; value: React.Ref<HTMLElement> }) {
  return (
    <div className="pw-tr-meter" data-channel={name}>
      <span className="pw-label">{label}</span>
      <span className="pw-tr-track"><i ref={fill} data-testid={`meter-${name}`} /></span>
      <b ref={value} className="pw-num pw-readout" data-testid={`meter-${name}-value`} />
    </div>
  );
}

function options(settings: Record<string, unknown>) {
  const channels = CHANNEL_ORDER.filter((c) => Array.isArray(settings.channels) && settings.channels.includes(c));
  return {
    reference: settings.reference === 'previous' ? 'previous' as const : 'best' as const,
    channels: channels.length ? channels : ['throttle', 'brake'] as Channel[],
  };
}

function Traces({ car, settings, width, height, units }: TileProps) {
  const runtime = useRuntime();
  const slow = useSlow((s) => s);
  const traces = lapTracesFor(runtime);
  const version = useSyncExternalStore(traces.subscribe, () => traces.version);
  const index = car && (car.status === 'ok' || car.status === 'retired') ? car.index : null;
  const info = index === null ? null : slow.state?.cars?.[index];
  const { reference, channels } = options(settings);
  const referenceLap = info?.[reference]?.lap_number;
  const host = useRef<HTMLDivElement>(null);
  const status = useRef<HTMLSpanElement>(null);
  const throttleFill = useRef<HTMLElement>(null);
  const throttleValue = useRef<HTMLElement>(null);
  const brakeFill = useRef<HTMLElement>(null);
  const brakeValue = useRef<HTMLElement>(null);
  const chart = useRef<TraceChart | null>(null);
  const inputs = useRef<InputHistory | null>(null);
  if (!inputs.current) inputs.current = new InputHistory();

  useEffect(() => {
    chart.current = createTraceChart(host.current!);
    return () => { chart.current?.destroy(); chart.current = null; };
  }, []);

  useEffect(() => {
    if (index !== null && referenceLap !== undefined) traces.request(index, referenceLap);
  }, [traces, index, referenceLap, slow.epoch, slow.connection]);

  const history = useRef(slow.history);
  useEffect(() => {
    const changed = slow.history.filter((entry) => entry.kind === 'lap' && !history.current.includes(entry));
    history.current = slow.history;
    if (!changed.length || index === null) return;
    traces.invalidate();
    for (const entry of changed) {
      if (entry.kind === 'lap' && entry.car === index) {
        traces.invalidate(index, entry.body.lap_number);
        traces.request(index, entry.body.lap_number);
      }
    }
    for (const n of [info?.previous?.lap_number, info?.best?.lap_number]) {
      if (n !== undefined) traces.request(index, n);
    }
  }, [slow.history, info, index, traces]);

  useFrameTile(tile.declaration, car, ({ frame, rings, car: carIndex, connected }) => {
    const state = carIndex === null ? 3 : effectiveState(frameReadoutState(frame, carIndex, 'throttle'), connected);
    const showing = carIndex !== null && (state === 0 || state === 1);
    const meter = (fill: HTMLElement | null, value: HTMLElement | null, percent: number) => {
      if (value) writeReadout(value, state as ReadoutState, `${percent}%`);
      if (fill) {
        const height = `${showing ? Math.min(100, percent) : 0}%`;
        if (fill.style.height !== height) fill.style.height = height;
        if (fill.dataset.state !== String(state)) fill.dataset.state = String(state);
      }
    };
    meter(throttleFill.current, throttleValue.current, showing ? frame.throttle[carIndex]! : 0);
    meter(brakeFill.current, brakeValue.current, showing ? frame.brake[carIndex]! : 0);
    const overlay = carIndex === null || referenceLap === undefined ? null : traces.get(carIndex, referenceLap) ?? null;
    inputs.current!.update(rings, `${frame.epoch || slow.epoch}:${carIndex}`);
    const plot = buildWindowPlot(inputs.current!.rings, overlay, channels, units);
    if (status.current) {
      status.current.hidden = plot.hasData && state === 0;
      status.current.textContent = state === 2 ? 'Restricted' : plot.hasData && state === 1 ? 'Stale ◷' : 'No data';
    }
    if (host.current) host.current.style.opacity = state === 1 ? '0.55' : '1';
    chart.current?.draw(plot, { width: Math.max(1, width - CHART_INSET.width), height: Math.max(1, height - CHART_INSET.height) });
  }, [slow, version, width, height, units, reference, channels.join(',')]);

  const legend = channels.map((c) => `${c[0]!.toUpperCase()}${c.slice(1)}`).join(' · ');
  return <div className="pw-tr">
    <TileTitle note={`${legend} · Dim ${reference === 'best' ? 'best lap' : 'previous lap'}`}>Driver inputs / last 30 s</TileTitle>
    <span className="pw-tr-status pw-label" role="status" ref={status}>No data</span>
    <div className="pw-tr-body">
      <div className="pw-tr-meters" aria-hidden="true">
        <Meter name="throttle" label="THR" fill={throttleFill} value={throttleValue} />
        <Meter name="brake" label="BRK" fill={brakeFill} value={brakeValue} />
      </div>
      <div ref={host} role="img" aria-label="Inputs over the last 30 seconds: bright thick current inputs, dim thin reference lap; teal throttle, pink brake" />
    </div>
  </div>;
}

function Settings({ settings, onChange }: TileSettingsProps) {
  const selected = options(settings);
  return <>
    <fieldset><legend>Reference lap</legend>{(['best', 'previous'] as const).map((r) =>
      <label key={r}><input type="radio" checked={selected.reference === r}
        onChange={() => onChange({ ...settings, reference: r })} />{r === 'best' ? 'Best lap' : 'Previous lap'}</label>)}</fieldset>
    <fieldset><legend>Channels</legend>{CHANNEL_ORDER.map((c) => <label key={c}>
      <input type="checkbox" checked={selected.channels.includes(c)} onChange={() => {
        const next = CHANNEL_ORDER.filter((key) => key === c ? !selected.channels.includes(key) : selected.channels.includes(key));
        if (next.length) onChange({ ...settings, channels: next });
      }} />{c[0]!.toUpperCase()}{c.slice(1)}</label>)}</fieldset>
  </>;
}

const tile = defineTileModule({
  declaration: {
    type: 'throttle-brake-traces', name: 'Throttle/brake traces', binding: 'follows-car', rate: 'frame',
    minSize: { w: 6, h: 4 }, defaultSize: { w: 10, h: 7 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 400, minHeight: 200 }],
    reads: [{ source: 'frame', fields: ['lapDistance', 'throttle', 'brake', 'speed'] }, { source: 'state', keys: ['cars'] }, { source: 'history', kinds: ['lap'] }],
  },
  Component: Traces, Settings,
});
export default tile;
