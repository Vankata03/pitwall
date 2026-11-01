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
  const running = stints.find((s) => s.body.end_lap === null)?.body;
  const runningCompound = running ? (status ? status.visual_tyre_compound : running.visual_compound) : null;
  const lapsOf = (stint: (typeof stints)[number]['body']) => {
    const end = stint.end_lap ?? (lap ? lap.current_lap_num - 1 : stint.start_lap - 1);
    return Math.max(0, end - stint.start_lap + 1);
  };
  const stintLaps = running && lap ? lapsOf(running) : null;
  return (
    <div className="pw-stint" data-density={density}>
      <TileTitle note={runningCompound === null ? undefined : compound(runningCompound)}>
        Current stint {car?.status === 'retired' ? '· retired' : ''}
      </TileTitle>
      <div className="pw-stint-head">
        <span className="pw-stint-count">
          <span className="pw-num pw-stint-laps" data-testid="stint-laps">
            {stintLaps === null ? '–' : String(stintLaps).padStart(2, '0')}
          </span>
          <span className="pw-label">Laps</span>
        </span>
        <span className="pw-stint-window">
          <span className="pw-label">Game pit window</span>
          <Value
            readout={pitWindow}
            stale={stale}
            format={(v) => `Laps ${v.ideal_lap}–${v.latest_lap} · rejoin P${v.rejoin_position}`}
          />
        </span>
      </div>
      <div className="pw-stint-blocks" data-testid="stint-blocks" aria-hidden="true">
        {stints.map(({ id, body: s }) => (
          <span
            key={id}
            className="pw-stint-group"
            data-compound={compoundKey(s.end_lap === null ? (runningCompound ?? s.visual_compound) : s.visual_compound)}
            style={{ flexGrow: Math.max(1, lapsOf(s)) }}
          >
            {Array.from({ length: lapsOf(s) }, (_, i) => (
              <i key={i} />
            ))}
          </span>
        ))}
      </div>
      <div className="pw-stint-caption">
        <span className="pw-label">Completed laps by stint</span>
        <span className="pw-label">Tyre age {status ? status.tyres_age_laps : '–'}</span>
      </div>
      <div className="pw-stint-detail">
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
                  <p className="pw-wear">
                    Wear{' '}
                    <Value
                      readout={wear}
                      stale={stale}
                      format={(v) => v.map((n) => `${Math.round(100 - n)}%`).join(' / ')}
                    />
                    {density !== 'compact' && <span className="pw-label"> RL / RR / FL / FR</span>}
                  </p>
                  {s.starting_age !== null && (
                    <p className="pw-label">
                      {s.starting_age > 0 ? `Used set · starting age ${s.starting_age}` : 'New set'}
                    </p>
                  )}
                </li>
              );
            })}
          </ol>
        ) : (
          <p>No stint history</p>
        )}
        {stops.length ? (
          <ol aria-label="Pit stops">
            {stops.map(({ id, body: stop }) => (
              <li key={id} data-stop="true">
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
          <p className="pw-label">No stops</p>
        )}
      </div>
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
