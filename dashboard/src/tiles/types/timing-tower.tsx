import { createContext, useContext, type CSSProperties } from 'react';
import type { CarState } from '../../generated/wire/CarState';
import type { Readout as JsonReadout } from '../../generated/wire/Readout';
import { useSlow } from '../../runtime/react';
import { jsonReadoutView } from '../../runtime/readout';
import { carLabel, participantsOf } from '../../session/cars';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';

// The session-wide host can supply device focus without giving the tower a car binding.
// Undefined means Follow player; null means a saved focus is not in this session.
const Focus = createContext<number | null | undefined>(undefined);
export const TowerFocusProvider = Focus.Provider;
const data = <T,>(r: JsonReadout<T> | undefined): T | undefined => (r && 'value' in r ? r.value : undefined);
const lapTime = (ms: number | undefined) =>
  ms ? `${Math.floor(ms / 60000)}:${((ms % 60000) / 1000).toFixed(3).padStart(6, '0')}` : '—';
const delta = (ms: number) => `${ms < 0 ? '−' : '+'}${(Math.abs(ms) / 1000).toFixed(3)}`;
const compound = (n: number) =>
  ({ 16: 'S', 17: 'M', 18: 'H', 7: 'I', 8: 'W', 19: 'SS', 20: 'S', 21: 'M', 22: 'H' })[n] ?? String(n);
function Cell<T>({
  readout,
  format,
  stale,
}: {
  readout: JsonReadout<T> | undefined;
  format: (v: T) => string;
  stale: boolean;
}) {
  const view = jsonReadoutView(readout ?? { state: 'absent' }, format);
  return (
    <td
      className="pw-readout"
      data-state={stale && view.state === 'live' ? 'stale' : view.state}
      title={view.text}
    >
      {view.text}
    </td>
  );
}
const options = (s: Record<string, unknown>) => ({
  gap: s.gap === 'leader' || s.gap === 'focus' ? s.gap : 'interval',
  rows: s.rows === 'full' || s.rows === 'around' ? s.rows : null,
  radius:
    typeof s.radius === 'number' && Number.isFinite(s.radius)
      ? Math.max(0, Math.min(12, Math.floor(s.radius)))
      : 3,
});
function Settings({ settings, onChange }: TileSettingsProps) {
  const s = options(settings);
  return (
    <>
      <label>
        Gap reference
        <select value={s.gap} onChange={(e) => onChange({ ...settings, gap: e.target.value })}>
          <option value="interval">Interval</option>
          <option value="leader">Leader</option>
          <option value="focus">Focus-relative</option>
        </select>
      </label>
      <label>
        Rows
        <select value={s.rows ?? 'auto'} onChange={(e) => onChange({ ...settings, rows: e.target.value })}>
          <option value="auto">Density default</option>
          <option value="full">Full field</option>
          <option value="around">Around focus</option>
        </select>
      </label>
      <label>
        Cars each side
        <input
          type="number"
          min="0"
          max="12"
          value={s.radius}
          onChange={(e) => onChange({ ...settings, radius: Number(e.target.value) })}
        />
      </label>
    </>
  );
}
function Tower({ settings, density, height }: TileProps) {
  const slow = useSlow((s) => s);
  const state = slow.state;
  const provided = useContext(Focus);
  const focus = provided === undefined ? state?.playerCar : provided;
  const session = data(state?.session);
  const all = participantsOf(state)
    .filter((p) => {
      const result = data(state?.cars[p.index]?.lap)?.result_status;
      return result !== 0 && result !== 1;
    })
    .sort((a, b) => {
      const dead = (i: number) => {
        const c = state?.cars[i];
        return !!c?.retired || [6, 7].includes(data(c?.lap)?.result_status ?? 0);
      };
      return Number(dead(a.index)) - Number(dead(b.index)) || (a.position ?? 99) - (b.position ?? 99);
    });
  if (session?.sessionType === 18)
    return <div className="pw-tile-message">Time Trial — only the player car runs</div>;
  if (!state || !all.length) return <div className="pw-tile-message">No timing data</div>;
  const race = [15, 16, 17].includes(session?.sessionType ?? -1);
  const s = options(settings);
  const full = density === 'full';
  const regular = density !== 'compact';
  const capacity = Math.max(
    0,
    Math.floor((height - (full ? 120 : regular ? 104 : 80)) / (full ? 72 : regular ? 48 : 28)),
  );
  const around = s.rows === 'around' || (s.rows === null && !regular) || all.length > capacity;
  const at = all.findIndex((p) => p.index === focus);
  let rows = all;
  if (around) {
    const center = Math.max(0, at);
    rows = all.slice(Math.max(0, center - s.radius), center + s.radius + 1);
    const pinnedLeader = !regular && all[0] && !rows.includes(all[0]) ? all[0] : null;
    const available = Math.max(0, capacity - (pinnedLeader ? 1 : 0));
    if (rows.length > available) {
      // Keep the nearest cars, retaining their classification order.
      const nearest = [...rows]
        .sort((a, b) => Math.abs(all.indexOf(a) - center) - Math.abs(all.indexOf(b) - center))
        .slice(0, available);
      rows = rows.filter((p) => nearest.includes(p));
    }
    if (pinnedLeader && capacity > 0) rows = [pinnedLeader, ...rows];
  }
  const leader = all.find((p) => p.position === 1) ?? all[0]!;
  const leaderCar = state.cars[leader.index];
  const focusCar = focus == null ? null : state.cars[focus];
  const best = (c: CarState | null | undefined) => c?.best?.lap_time_ms;
  const reference = s.gap === 'focus' ? (race ? focusCar : best(focusCar) ? focusCar : leaderCar) : leaderCar;
  const gapLabel =
    s.gap === 'focus'
      ? `Gap to ${reference === focusCar && focusCar ? (all.find((p) => p.index === focus) ? carLabel(all.find((p) => p.index === focus)!) : 'focus') : race ? 'focus' : 'session best'}`
      : race
        ? s.gap === 'leader'
          ? 'Leader'
          : 'Interval'
        : 'Session best';
  const stale = slow.connection !== 'open' || state.lifecycle === 'Lost';
  let band = ['', 'Safety car', 'Virtual safety car', 'Formation lap'][session?.safetyCarStatus ?? 0] ?? '';
  // Event chronology decides when a red flag is superseded by a restart or lights out.
  for (const entry of slow.history) {
    if (entry.kind !== 'raceControl' || entry.body.rewound) continue;
    const kind = entry.body.kind;
    if (typeof kind === 'object' && 'Game' in kind) {
      const detail = kind.Game.details;
      if (detail === 'RedFlag') band = 'Red flag';
      if (detail === 'LightsOut' || detail === 'SessionStarted')
        band = ['', 'Safety car', 'Virtual safety car', 'Formation lap'][session?.safetyCarStatus ?? 0] ?? '';
    }
  }
  return (
    <div className="pw-tower" style={{ overflow: 'hidden', height: '100%', padding: 8 }}>
      <style>{`.pw-tower table{width:100%;border-collapse:collapse;table-layout:fixed}.pw-tower tr{display:grid;grid-template-columns:24px minmax(60px,1.5fr) minmax(72px,1.5fr) 30px;align-items:center;min-height:28px;border-bottom:1px solid var(--pw-line)}.pw-tower[data-density=regular] tr,.pw-tower[data-density=full] tr{grid-template-columns:repeat(12,minmax(0,1fr));min-height:48px}.pw-tower[data-density=full] tr{min-height:72px}.pw-tower[data-density] tr>:nth-child(1){grid-column:span 1}.pw-tower[data-density] tr>:nth-child(2){grid-column:span 4}.pw-tower[data-density] tr>:nth-child(3){grid-column:span 5}.pw-tower[data-density] tr>:nth-child(4){grid-column:span 2}.pw-tower[data-density] tr>:nth-child(5),.pw-tower[data-density] tr>:nth-child(6),.pw-tower[data-density] tr>:nth-child(7){grid-column:span 2}.pw-tower[data-density] tr>:nth-child(8){grid-column:span 6}.pw-tower[data-density] tr>:nth-child(9),.pw-tower[data-density] tr>:nth-child(10),.pw-tower[data-density] tr>:nth-child(11){grid-column:span 3}.pw-tower[data-density] tr>:nth-child(12){grid-column:span 2}.pw-tower[data-density] tr>:nth-child(13){grid-column:span 1}.pw-tower[data-density=compact] tr{grid-template-columns:repeat(12,minmax(0,1fr))}.pw-tower th{font-size:var(--pw-text-label);color:var(--pw-text-muted);font-weight:400}.pw-tower td{font-size:var(--pw-text-value)}.pw-tower td,.pw-tower th{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;text-align:left;padding:2px}.pw-tower [aria-current=true]{background:var(--pw-line);border-left:3px solid var(--pw-accent)}.pw-tower [data-retired=true]{color:var(--pw-text-muted)}.pw-tower [data-state=stale]{color:var(--pw-text-muted)}`}</style>
      <div className="pw-label">Timing tower</div>
      <div role="status" className="pw-label" data-state={stale ? 'stale' : state.session.state}>
        {band || 'Racing'}
      </div>
      <div className="pw-tower" data-density={density}>
        <table aria-label="Timing tower">
          <thead>
            <tr>
              {[
                'Pos',
                'Driver',
                gapLabel,
                'Tyre',
                ...(regular ? ['Team', 'Age', 'Stops', 'Status'] : []),
                ...(full ? ['Last lap', 'Best lap', 'Penalties / warnings', 'Wear (max)', 'Gained'] : []),
              ].map((h) => (
                <th scope="col" key={h} title={h}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((p) => {
              const c = state.cars[p.index]!;
              const lap = data(c.lap);
              const retired = c.retired || [6, 7].includes(lap?.result_status ?? 0);
              const gap = data(c.gaps);
              let gapText = '—';
              let gapState = c.gaps.state;
              if (race && gap) {
                const compare =
                  s.gap === 'interval'
                    ? state.cars[all.find((a) => a.position === (p.position ?? 0) - 1)?.index ?? leader.index]
                    : reference;
                const other = data(compare?.gaps);
                if (other) {
                  const laps =
                    gap.laps_behind === null || other.laps_behind === null
                      ? null
                      : Math.abs(gap.laps_behind - other.laps_behind);
                  gapText = laps
                    ? `+${laps} LAP`
                    : p.position === 1 && s.gap !== 'focus'
                      ? 'Leader'
                      : s.gap === 'interval'
                        ? gap.to_car_in_front_ms === null
                          ? '—'
                          : delta(gap.to_car_in_front_ms)
                        : delta(gap.to_leader_ms - other.to_leader_ms);
                  if (compare?.gaps.state === 'stale') gapState = 'stale';
                }
              } else if (!race && best(c)) {
                gapText =
                  p.position === 1
                    ? lapTime(best(c))
                    : best(reference)
                      ? delta(best(c)! - best(reference)!)
                      : '—';
                gapState = c.lap.state;
              }
              const colour = data(c.participant)?.livery_colour[0];
              return (
                <tr
                  key={p.index}
                  aria-current={p.index === focus ? 'true' : undefined}
                  data-retired={retired}
                >
                  <Cell readout={c.lap} stale={stale} format={(v) => String(v.car_position)} />
                  <Cell readout={c.participant} stale={stale} format={() => carLabel(p)} />
                  <td
                    className="pw-readout"
                    data-state={stale && gapState === 'live' ? 'stale' : gapState}
                    title={gapText}
                  >
                    {gapText}
                  </td>
                  <Cell readout={c.status} stale={stale} format={(v) => compound(v.visual_tyre_compound)} />
                  {regular && (
                    <>
                      <td>
                        <span
                          aria-label="Team colour"
                          style={
                            {
                              display: 'inline-block',
                              width: 16,
                              height: 4,
                              background: colour
                                ? `rgb(${colour.red},${colour.green},${colour.blue})`
                                : 'var(--pw-text-muted)',
                            } as CSSProperties
                          }
                        />
                      </td>
                      <Cell readout={c.status} stale={stale} format={(v) => String(v.tyres_age_laps)} />
                      <Cell readout={c.lap} stale={stale} format={(v) => String(v.num_pit_stops)} />
                      <Cell
                        readout={c.lap}
                        stale={stale}
                        format={(v) =>
                          v.result_status === 6
                            ? 'DSQ'
                            : retired
                              ? 'Retired'
                              : v.pit_status
                                ? 'PIT'
                                : 'Running'
                        }
                      />
                    </>
                  )}
                  {full && (
                    <>
                      <Cell readout={c.lap} stale={stale} format={(v) => lapTime(v.last_lap_time_in_ms)} />
                      <td
                        className="pw-readout"
                        data-state={stale || c.lap.state === 'stale' ? 'stale' : best(c) ? 'live' : 'absent'}
                      >
                        {lapTime(best(c))}
                      </td>
                      <Cell
                        readout={c.penalties}
                        stale={stale}
                        format={(v) =>
                          `${v.seconds}s / ${v.corner_cutting_warnings} TL${v.drive_throughs ? ` / ${v.drive_throughs} DT` : ''}${v.stop_gos ? ` / ${v.stop_gos} SG` : ''}`
                        }
                      />
                      <Cell
                        readout={c.tyreCondition}
                        stale={stale}
                        format={(v) => `${Math.round(100 - Math.min(...v))}%`}
                      />
                      <Cell
                        readout={c.lap}
                        stale={stale}
                        format={(v) =>
                          v.grid_position > 0
                            ? `${v.grid_position - v.car_position >= 0 ? '+' : ''}${v.grid_position - v.car_position}`
                            : '—'
                        }
                      />
                    </>
                  )}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
export default defineTileModule({
  declaration: {
    type: 'timing-tower',
    name: 'Timing tower',
    binding: 'session-wide',
    rate: 'slow',
    minSize: { w: 4, h: 6 },
    defaultSize: { w: 6, h: 12 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 320, minHeight: 240 },
      { density: 'full', minWidth: 460, minHeight: 480 },
    ],
    reads: [
      { source: 'state', keys: ['cars', 'session', 'playerCar'] },
      { source: 'history', kinds: ['raceControl'] },
    ],
  },
  Component: Tower,
  Settings,
});
