import type { Readout } from '../../generated/wire/Readout';
import { useSlow } from '../../runtime/react';
import { jsonReadoutView } from '../../runtime/readout';
import { defineTileModule, type TileProps } from '../contract';
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
function Stint({ car, density }: TileProps) {
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
  return (
    <div className="pw-stint" style={{ padding: 8, height: '100%', overflow: 'auto' }}>
      <style>{`.pw-stint [data-state=stale]{color:var(--pw-text-muted)}.pw-stint ol{list-style:none;padding:0;margin:8px 0;display:flex;flex-wrap:wrap;gap:8px}.pw-stint li{border:1px solid var(--pw-line);padding:8px;flex:1 1 140px;min-width:0}.pw-stint p{margin:4px 0}.pw-stint [data-current=true]{border-top:3px solid var(--pw-accent)}.pw-stint .pw-wear{overflow-wrap:anywhere}`}</style>
      <div className="pw-label">Stints {car?.status === 'retired' ? '· retired' : ''}</div>
      {stints.length ? (
        <ol aria-label="Tyre stints">
          {stints.map(({ id, body: s }) => {
            const active = s.end_lap === null;
            const end = s.end_lap ?? lap?.current_lap_num;
            const wear = condition(active ? current.tyreCondition : s.tyre_condition);
            return (
              <li key={id} data-current={active}>
                <strong>
                  {compound(active && status ? status.visual_tyre_compound : s.visual_compound)}
                </strong>
                <p
                  className="pw-readout"
                  data-state={stale || current.lap.state === 'stale' ? 'stale' : 'live'}
                >
                  Laps {s.start_lap}–{end ?? '—'} ·{' '}
                  {active ? 'current' : `${Math.max(0, (end ?? s.start_lap - 1) - s.start_lap + 1)} laps`}
                </p>
                <p>
                  Tyre age{' '}
                  <span
                    className="pw-readout"
                    data-state={
                      stale || (active && current.status.state === 'stale')
                        ? 'stale'
                        : s.tyre_age_laps === null
                          ? 'absent'
                          : 'live'
                    }
                  >
                    {active && status ? status.tyres_age_laps : (s.tyre_age_laps ?? '—')}
                  </span>
                </p>
                {s.starting_age !== null && (
                  <p className="pw-label">
                    {s.starting_age > 0 ? `Used set · starting age ${s.starting_age}` : 'New set'}
                  </p>
                )}
                <p className="pw-wear">
                  Wear{' '}
                  <Value
                    readout={wear}
                    stale={stale}
                    format={(v) => v.map((n) => `${Math.round(100 - n)}%`).join(' / ')}
                  />
                </p>
                {density !== 'compact' && <span className="pw-label">RL / RR / FL / FR</span>}
              </li>
            );
          })}
        </ol>
      ) : (
        <p>No stint history</p>
      )}
      <div className="pw-label">Stops</div>
      {stops.length ? (
        <ol aria-label="Pit stops">
          {stops.map(({ id, body: stop }) => (
            <li key={id}>
              <p>Stop · lap {stop.lap_number}</p>
              <p className="pw-readout" data-state={stale ? 'stale' : 'live'}>
                {stop.stopped_time_ms === null ? '—' : (stop.stopped_time_ms / 1000).toFixed(3)}s stopped ·{' '}
                {stop.lane_time_ms === null ? '—' : (stop.lane_time_ms / 1000).toFixed(3)}s lane
              </p>
              {stop.exit_time === null && <span className="pw-label">In pit lane</span>}
            </li>
          ))}
        </ol>
      ) : (
        <p>No stops</p>
      )}
      <div className="pw-label">Game pit window</div>
      <Value
        readout={pitWindow}
        stale={stale}
        format={(v) => `Laps ${v.ideal_lap}–${v.latest_lap} · rejoin P${v.rejoin_position}`}
      />
    </div>
  );
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
