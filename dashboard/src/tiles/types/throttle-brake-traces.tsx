import { useEffect, useRef, useSyncExternalStore } from 'react';
import { useRuntime, useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState } from '../../runtime/readout';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import { useFrameTile } from '../use-frame-tile';
import { CurrentLap } from '../traces/current-lap';
import { lapTracesFor } from '../traces/lap-traces';
import { buildPlot, CHANNEL_ORDER, type Channel } from '../traces/plot';
import { createTraceChart, type TraceChart } from '../traces/chart';

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
  const lap = info?.lap && 'value' in info.lap ? info.lap.value.current_lap_num : null;
  const { reference, channels } = options(settings);
  const referenceLap = info?.[reference]?.lap_number;
  const host = useRef<HTMLDivElement>(null);
  const status = useRef<HTMLSpanElement>(null);
  const chart = useRef<TraceChart | null>(null);
  const live = useRef({ identity: '', lap: new CurrentLap(), consumed: 0, seeded: null as object | null });

  useEffect(() => {
    chart.current = createTraceChart(host.current!);
    return () => { chart.current?.destroy(); chart.current = null; };
  }, []);

  useEffect(() => {
    if (index !== null && referenceLap !== undefined) traces.request(index, referenceLap);
  }, [traces, index, referenceLap, slow.epoch, slow.connection]);

  useEffect(() => {
    if (index === null || lap === null) return;
    if (live.current.lap.joinedMidLap) traces.request(index, lap);
  }, [runtime, traces, index, lap, slow.epoch]);

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
    const current = live.current;
    const identity = `${frame.epoch || slow.epoch}:${carIndex}`;
    if (current.identity !== identity) {
      current.identity = identity;
      current.lap.reset();
      current.consumed = rings ? rings.lapDistance.version - rings.lapDistance.length : 0;
      current.seeded = null;
    }
    if (rings) {
      const ring = rings.lapDistance;
      const added = Math.min(ring.length, Math.max(0, ring.version - current.consumed));
      for (let i = ring.length - added; i < ring.length; i++) {
        current.lap.record(ring.at(i), rings.throttle.at(i), rings.brake.at(i), rings.speed.at(i));
      }
      current.consumed = ring.version;
      if (carIndex !== null && lap !== null && current.lap.joinedMidLap) {
        traces.request(carIndex, lap);
        const seed = traces.get(carIndex, lap);
        if (seed && current.seeded !== seed) { current.lap.seed(seed); current.seeded = seed; }
      }
    }
    const overlay = carIndex === null || referenceLap === undefined ? null : traces.get(carIndex, referenceLap) ?? null;
    const plot = buildPlot({ current: current.lap.samples(), reference: overlay, channels, units });
    if (status.current) {
      status.current.hidden = plot.hasData && state === 0;
      status.current.textContent = state === 2 ? 'Restricted' : plot.hasData && state === 1 ? 'Stale ◷' : 'No data';
    }
    if (host.current) host.current.style.opacity = state === 1 ? '0.55' : '1';
    chart.current?.draw(plot, { width: Math.max(1, width - 16), height: Math.max(1, height - 64) });
  }, [slow, version, width, height, units, reference, channels.join(',')]);

  return <div style={{ height: '100%', overflow: 'hidden', padding: 8 }}>
    <div className="pw-label">Throttle/brake traces · {reference === 'best' ? 'Best lap' : 'Previous lap'}</div>
    <div className="pw-label">{channels.map((c) => `${c[0]!.toUpperCase()}${c.slice(1)}`).join(' · ')} · Solid current / dashed reference</div>
    <span role="status" ref={status}>No data</span>
    <div ref={host} role="img" aria-label="Lap traces against distance: solid current lap, dashed reference" />
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
