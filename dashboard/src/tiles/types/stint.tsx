import type { Readout } from '../../generated/wire/Readout';
import { useSlow } from '../../runtime/react';
import { jsonReadoutView } from '../../runtime/readout';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import { TileTitle } from '../frame-kit';
import '../stint.css';
const data = <T,>(r: Readout<T> | undefined): T | undefined => (r && 'value' in r ? r.value : undefined);
const compound = (n: number) =>
  ({
    16: 'Soft',
    17: 'Medium',
    18: 'Hard',
    7: 'Intermediate',
    8: 'Wet',
    19: 'Super soft',
    20: 'Soft',
    21: 'Medium',
    22: 'Hard',
  })[n] ?? `Compound ${n}`;
/** Block colour by compound. The compound is always named in text too, so colour only repeats it. */
const compoundKey = (n: number): string =>
  ({ 16: 'soft', 19: 'soft', 20: 'soft', 17: 'medium', 21: 'medium', 18: 'hard', 22: 'hard', 7: 'inter', 8: 'wet' })[n] ?? 'unknown';
function Value<T>({
  readout,
  format,
  stale,
}: {
  readout: Readout<T> | undefined;
  format: (v: T) => string;
  stale: boolean;
}) {
  const v = jsonReadoutView(readout ?? { state: 'absent' }, format);
  return (
    <span className="pw-readout" data-state={stale && v.state === 'live' ? 'stale' : v.state}>
      {v.text}
    </span>
  );
}
function TyreWear({ readout, stale }: { readout: Readout<[number, number, number, number]>; stale: boolean }) {
  if (!('value' in readout)) return <Value readout={readout} stale={stale} format={() => ''} />;
  return <div className="pw-stint-wheels" aria-label="Tyre wear">
    {(['FL', 'FR', 'RL', 'RR'] as const).map((wheel, i) => {
      const remaining = readout.value[[2, 3, 0, 1][i]!]!;
      const wear = Math.max(0, Math.min(100, Math.round(100 - remaining)));
      return <div key={wheel}>
        <span className="pw-label">{wheel}</span>
        <b><Value readout={{ state: readout.state, value: wear }} stale={stale} format={v => `${v}%`} /></b>
        <span className="pw-stint-wear-track" aria-hidden="true"><i style={{ width: `${wear}%` }} /></span>
      </div>;
    })}
  </div>;
}

function Stint({ car }: TileProps) {
  const slow = useSlow((s) => s);
  const state = slow.state;
  const index = car && 'index' in car ? car.index : null;
  const current = index === null ? null : state?.cars[index];
  const entries = slow.history.filter((e) => e.car === index);
  const stints = entries
    .filter((e) => e.kind === 'stint')
    .sort((a, b) => a.body.start_lap - b.body.start_lap);
  const stops = entries
    .filter((e) => e.kind === 'pitStop')
    .sort((a, b) => a.body.lap_number - b.body.lap_number);
  const stale = slow.connection !== 'open' || state?.lifecycle === 'Lost';
  const lap = data(current?.lap);
  const status = data(current?.status);
  // Never expose a historical value after the car switches telemetry to restricted.
  const condition = (r: Readout<[number, number, number, number]>) =>
    current?.tyreCondition.state === 'restricted' ? current.tyreCondition : r;
  const pitWindow =
    index !== null && index === state?.playerCar && data(state.pitWindow)?.car_index === index
      ? state.pitWindow
      : ({ state: 'absent' } as const);
  if (!current) return <div className="pw-tile-message">No stint data</div>;
  const running = stints.find((s) => s.body.end_lap === null)?.body;
  const runningCompound = running ? (status ? status.visual_tyre_compound : running.visual_compound) : null;
  const lapsOf = (stint: (typeof stints)[number]['body']) => {
    const end = stint.end_lap ?? (lap ? lap.current_lap_num - 1 : stint.start_lap - 1);
    return Math.max(0, end - stint.start_lap + 1);
  };
  const stintLaps = running && lap ? lapsOf(running) : null;
  const symbol = (n: number) => ({ soft: 'S', medium: 'M', hard: 'H', inter: 'I', wet: 'W' })[compoundKey(n)] ?? '?';
  const completed = stints.filter(s => s.body.end_lap !== null).reverse();
  const setLabel = (age: number | null) => age === null ? '' : age > 0 ? `Used set · starting age ${age}` : 'New set';
  return <div className="pw-stint">
    <TileTitle note={`${stops.length} ${stops.length === 1 ? 'stop' : 'stops'}`}>
      Current stint {car?.status === 'retired' ? '· retired' : ''}
    </TileTitle>
    {running && runningCompound !== null ? <section className="pw-stint-active" data-compound={compoundKey(runningCompound)} aria-label="Current tyre set">
      <div className="pw-stint-summary">
        <span className="pw-stint-tyre" aria-hidden="true">{symbol(runningCompound)}</span>
        <div className="pw-stint-set-name">
          <strong>{compound(runningCompound)}</strong>
          <span className="pw-label">{setLabel(running.starting_age)}</span>
        </div>
        <div className="pw-stint-metric">
          <b className="pw-num" data-testid="stint-laps">{stintLaps === null ? '–' : String(stintLaps).padStart(2, '0')}</b>
          <span className="pw-label">Laps done</span>
        </div>
        <div className="pw-stint-metric">
          <b className="pw-num"><Value readout={current.status} stale={stale} format={v => String(v.tyres_age_laps)} /></b>
          <span className="pw-label">Tyre age</span>
        </div>
      </div>
      <div className="pw-stint-active-caption">
        <span className="pw-label">Laps {running.start_lap}–{lap?.current_lap_num ?? '—'} · current</span>
        <span className="pw-label">Tyre wear</span>
      </div>
      <TyreWear readout={condition(current.tyreCondition)} stale={stale} />
    </section> : null}
    {pitWindow.state !== 'absent' ? <div className="pw-stint-window">
      <span className="pw-label">Game pit window</span>
      <Value readout={pitWindow} stale={stale} format={v => `Laps ${v.ideal_lap}–${v.latest_lap} · rejoin P${v.rejoin_position}`} />
    </div> : null}
    <div className="pw-stint-detail">
      <ol aria-label="Tyre stints">
        {completed.map(({ id, body: s }) => {
          const wear = condition(s.tyre_condition);
          return <li className="pw-stint-past" key={id} data-compound={compoundKey(s.visual_compound)}>
            <details>
              <summary>
                <span className="pw-stint-tyre" aria-hidden="true">{symbol(s.visual_compound)}</span>
                <span className="pw-stint-past-name"><strong>{compound(s.visual_compound)}</strong><span>Laps {s.start_lap}–{s.end_lap} · {lapsOf(s)} {lapsOf(s) === 1 ? 'lap' : 'laps'}</span></span>
                <span className="pw-stint-final-wear"><Value readout={wear} stale={stale} format={v => `${Math.round(100 - Math.min(...v))}% max wear`} /></span>
              </summary>
              <div className="pw-stint-past-expanded">
                <div className="pw-stint-active-caption"><span className="pw-label">{setLabel(s.starting_age)}</span><span className="pw-label">Final tyre age {s.tyre_age_laps ?? '—'}</span></div>
                {'value' in wear ? <TyreWear readout={wear} stale={stale} /> : null}
              </div>
            </details>
          </li>;
        })}
      </ol>
      {!stints.length ? <p className="pw-label">No stint history</p> : !completed.length ? <p className="pw-label">No previous sets recorded</p> : null}
      {stops.length ? <ol className="pw-stint-stops" aria-label="Pit stops">
        {stops.map(({ id, body: stop }) => <li key={id}>
          <span className="pw-stint-stop-label">Stop · lap {stop.lap_number}{stop.exit_time === null ? <small className="pw-label">In pit lane</small> : null}</span>
          <div><span className="pw-label">Stopped</span><b className="pw-readout pw-num" data-state={stale ? 'stale' : 'live'}>{stop.stopped_time_ms === null ? '—' : `${(stop.stopped_time_ms / 1000).toFixed(3)} s`}</b></div>
          <div><span className="pw-label">Pit lane</span><b className="pw-readout pw-num" data-state={stale ? 'stale' : 'live'}>{stop.lane_time_ms === null ? '—' : `${(stop.lane_time_ms / 1000).toFixed(3)} s`}</b></div>
        </li>)}
      </ol> : null}
    </div>
  </div>;
}

export default defineTileModule({
  declaration: {
    type: 'stint',
    name: 'Stint',
    binding: 'follows-car',
    rate: 'slow',
    minSize: { w: 6, h: 4 },
    defaultSize: { w: 8, h: 7 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 300, minHeight: 240 },
    ],
    reads: [
      { source: 'state', keys: ['cars', 'playerCar', 'pitWindow'] },
      { source: 'history', kinds: ['stint', 'pitStop'] },
    ],
  },
  Component: Stint,
});
