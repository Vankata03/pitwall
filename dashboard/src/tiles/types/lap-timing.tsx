import { useRef } from 'react';
import type { CompletedLap } from '../../generated/wire/CompletedLap';
import type { HistoryEntry } from '../../generated/wire/HistoryEntry';
import { useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import { setData, showMessage, signed, slowState, StateReadout, TileMessage, TIME_TRIAL, useCarState, useConnected, useLifecycle, useSessionType, valueOf, write } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';
import '../frame-tiles.css';

const declaration = {
  type: 'lap-timing', name: 'Lap timing', binding: 'follows-car', rate: 'frame',
  minSize: { w: 4, h: 3 }, defaultSize: { w: 6, h: 4 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 300, minHeight: 150 },
    { density: 'full', minWidth: 400, minHeight: 220 },
  ],
  reads: [{ source: 'frame', fields: ['currentLapTime', 'deltaBest', 'deltaPrevious'] }, { source: 'state', keys: ['cars', 'session'] }, { source: 'history', kinds: ['lap', 'sector'] }],
} as const;
const references = { best: 'Best lap', previous: 'Previous lap', 'personal-best': 'Personal best', rival: 'Rival' };
type Reference = keyof typeof references;
const referenceOf = (settings: Record<string, unknown>): Reference =>
  typeof settings.reference === 'string' && Object.hasOwn(references, settings.reference) ? settings.reference as Reference : 'best';
const time = (ms: number, lap = false): string => {
  const seconds = Math.floor(ms / 1000) % 60;
  return `${lap || ms >= 60000 ? `${Math.floor(ms / 60000)}:${String(seconds).padStart(2, '0')}` : seconds}.${String(ms % 1000).padStart(3, '0')}`;
};
const deltaText = (ms: number) => signed(ms / 1000, 3);

/** Rank against earlier valid sectors, excluding the displayed lap itself. */
function rankSector(history: readonly HistoryEntry[], car: number | null, lap: number, sector: number, ms: number, valid: boolean): string {
  if (!valid) return 'slower';
  let personal = Infinity;
  let session = Infinity;
  for (const entry of history) {
    if (entry.car === car && (entry.kind === 'lap' || entry.kind === 'sector') && entry.body.lap_number >= lap) continue;
    let candidate: number | null = null;
    if (entry.kind === 'sector' && entry.body.sector === sector && entry.body.valid) candidate = entry.body.time_ms;
    if (entry.kind === 'lap' && entry.body.sector_valid[sector]) candidate = entry.body.sectors_ms[sector] ?? null;
    if (candidate === null || candidate <= 0) continue;
    session = Math.min(session, candidate);
    if (entry.car === car) personal = Math.min(personal, candidate);
  }
  return ms < session ? 'session-best' : ms <= personal ? 'personal-best' : 'slower';
}

function LapTiming({ car, settings, density }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const clock = useRef<HTMLSpanElement>(null);
  const delta = useRef<HTMLSpanElement>(null);
  const c = useCarState(car);
  const connected = useConnected();
  const lifecycle = useLifecycle();
  const sessionType = useSessionType();
  const history = useSlow(s => s.history);
  const reference = referenceOf(settings);
  const extra = reference === 'personal-best' || reference === 'rival';
  const lap = valueOf(c?.lap);
  const lapState = slowState(c?.lap, connected);
  const index = car && (car.status === 'ok' || car.status === 'retired') ? car.index : null;
  const result = reference === 'previous' ? c?.completedPrevious : c?.completedBest;
  const completed = result && lap && result.lap_number === lap.current_lap_num - 1 && lap.sector === 0 && lap.sector1_time_ms_part === 0 && lap.sector1_time_minutes_part === 0 ? result : null;
  const completedLap = completed ? [c?.previous, c?.best, ...history.filter(e => e.kind === 'lap' && e.car === index).map(e => e.body as CompletedLap)].find(l => l?.lap_number === completed.lap_number) : null;
  const tt = extra && sessionType === TIME_TRIAL ? reference === 'rival' ? c?.rival : c?.personalBest : null;
  const refNumber = completed?.reference_lap_number;
  const refLap = extra ? null : completed ? [c?.best, c?.previous, ...history.filter(e => e.kind === 'lap' && e.car === index).map(e => e.body as CompletedLap)].find(l => l?.lap_number === refNumber) : reference === 'previous' ? c?.previous : c?.best;
  const refTime = extra ? tt?.lap_time_in_ms : refLap?.lap_time_ms;
  const refSectors = extra ? tt ? [tt.sector1_time_in_ms, tt.sector2_time_in_ms, tt.sector3_time_in_ms] : [] : refLap?.sectors_ms ?? [];
  const sectors = completed ? completedLap?.sectors_ms ?? [null, null, null] : [
    lap && (lap.sector > 0 || lap.sector1_time_ms_part > 0 || lap.sector1_time_minutes_part > 0) ? lap.sector1_time_minutes_part * 60000 + lap.sector1_time_ms_part || null : null,
    lap ? lap.sector2_time_minutes_part * 60000 + lap.sector2_time_ms_part || null : null,
    null,
  ];
  const invalid = completed ? !completed.valid : lap?.current_lap_invalid === 1;
  const summaryState = connected ? ReadoutState.Live : ReadoutState.Stale;
  const sectorStates = sectors.map(ms => ms === null ? (lapState >= 2 ? lapState : ReadoutState.Absent) : completed ? summaryState : lapState);

  useFrameTile(declaration, car, ({ frame, car: i, connected: online }) => {
    const clockState = completed ? effectiveState(ReadoutState.Live, online) : i === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, i, 'currentLapTime'), online);
    const field = reference === 'previous' ? 'deltaPrevious' : 'deltaBest';
    // Keep the official time and sectors, but resume the running delta after five seconds.
    const holdDelta = completed && (extra || i === null || frame.currentLapTime[i]! < 5000);
    const difference = holdDelta ? extra ? refTime ? completed.lap_time_ms - refTime : null : completed.delta_ms : i === null || extra ? null : frame[field][i]!;
    const deltaState = difference === null ? ReadoutState.Absent : holdDelta ? effectiveState(ReadoutState.Live, online) : effectiveState(frameReadoutState(frame, i!, field), online);
    write(clock.current, clockState, time(completed?.lap_time_ms ?? (i === null ? 0 : frame.currentLapTime[i]!), true));
    write(delta.current, deltaState, deltaText(difference ?? 0));
    setData(delta.current, 'sign', deltaState < 2 ? difference! < 0 ? 'ahead' : difference! > 0 ? 'behind' : 'equal' : '');
    const states = [clockState, deltaState, ...(density !== 'compact' ? sectorStates : []), ...(density === 'full' && refTime ? [summaryState] : [])];
    showMessage(root.current, wholeTileMessage(states, { lifecycle, connected: online }));
  }, [c, reference, density, history, connected, lifecycle, sessionType]);

  return <div className="pw-frame-tile pw-lt" ref={root} data-density={density}>
    <div className="pw-frame-body">
      <div className="pw-lt-head">
        <div className="pw-stat"><span className="pw-label" data-testid="lt-delta-label">{references[reference]}</span><span ref={delta} className="pw-num pw-readout pw-lt-delta" data-testid="lt-delta" /></div>
        <div className="pw-stat"><span className="pw-label" data-testid="lt-lap-label">{lap ? `Lap ${completed?.lap_number ?? lap.current_lap_num}${completed ? ' complete' : ''}` : 'Lap time'}</span><span ref={clock} className="pw-num pw-readout pw-lt-time" data-testid="lt-time" /><span className="pw-label" data-testid="lt-invalid">{invalid ? 'Invalid' : ''}</span></div>
      </div>
      {density !== 'compact' ? <div className="pw-lt-sectors">{sectors.map((ms, s) => {
        const sectorState = sectorStates[s]!;
        const validEntry = history.find(e => e.kind === 'sector' && e.car === index && e.body.lap_number === lap?.current_lap_num && e.body.sector === s);
        const valid = completed ? completedLap?.sector_valid[s] ?? completed.valid : validEntry?.kind === 'sector' ? validEntry.body.valid : !invalid;
        const rank = ms === null || sectorState >= 2 ? '' : rankSector(history, index, completed?.lap_number ?? lap?.current_lap_num ?? 0, s, ms, valid);
        const d = completed && !extra ? completed.sector_deltas_ms[s] : ms !== null && refSectors[s] ? ms - refSectors[s]! : null;
        return <div className="pw-stat" key={s} data-testid={`lt-s${s + 1}`} data-rank={rank} data-current={String(!completed && lap?.sector === s)}>
          <span className="pw-label">S{s + 1}</span>
          <StateReadout state={sectorState} text={ms === null ? '' : `${time(ms)}${rank === 'session-best' ? ' ◆' : rank === 'personal-best' ? ' ●' : ''}`} testId={`lt-s${s + 1}-time`} />
          <StateReadout state={d == null ? ReadoutState.Absent : sectorState} text={deltaText(d ?? 0)} testId={`lt-s${s + 1}-delta`} />
        </div>;
      })}</div> : null}
      {completed && density !== 'compact' ? <div className="pw-lt-result"><span className="pw-label">Total lap result</span><span className="pw-num">{time(completed.lap_time_ms, true)}</span></div> : null}
      {density === 'full' ? <>
        {extra ? <div className="pw-lt-ref-sectors">{[0, 1, 2].map((s) => <div className="pw-stat" key={s}><span className="pw-label">REF S{s + 1}</span><StateReadout state={refSectors[s] ? summaryState : ReadoutState.Absent} text={time(refSectors[s] ?? 0)} testId={`lt-ref-s${s + 1}`} /></div>)}</div> : null}
        <div className="pw-lt-reference"><span className="pw-label" data-testid="lt-ref-label">REF LAP {refLap?.lap_number ?? '—'} · {references[reference]}</span><StateReadout state={refTime ? summaryState : ReadoutState.Absent} text={time(refTime ?? 0, true)} testId="lt-ref-time" /></div>
      </> : null}
    </div><TileMessage />
  </div>;
}

function Settings({ settings, onChange }: TileSettingsProps) {
  const tt = useSessionType() === TIME_TRIAL;
  const reference = referenceOf(settings);
  return <label>Reference lap<select aria-label="Reference lap" value={reference} onChange={e => onChange({ ...settings, reference: e.target.value })}>
    {Object.entries(references).map(([key, label]) => <option key={key} value={key} disabled={!tt && (key === 'personal-best' || key === 'rival')}>{label}{key === 'personal-best' || key === 'rival' ? ' (Time Trial)' : ''}</option>)}
  </select></label>;
}
export default defineTileModule({ declaration, Component: LapTiming, Settings });
