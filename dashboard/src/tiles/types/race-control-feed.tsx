import type { Event } from '../../generated/wire/Event';
import type { RaceControlKind } from '../../generated/wire/RaceControlKind';
import { useSlow } from '../../runtime/react';
import { carLabel, participantsOf, type Participant } from '../../session/cars';
import { raceStatus } from '../../session/race-status';
import type { WireHistoryEntry } from '../../runtime/wire';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import '../slow.css';

const types = ['Game', 'FlagChanged', 'MarshalFlagChanged', 'SafetyCarChanged', 'WeatherChanged', 'PitEntry', 'PitExit', 'Flashback'] as const;
const labels: Record<string, string> = { Game: 'Game events', FlagChanged: 'Flag', MarshalFlagChanged: 'Marshal flag', SafetyCarChanged: 'Safety car', WeatherChanged: 'Weather', PitEntry: 'Pit entry', PitExit: 'Pit exit', Flashback: 'Flashback' };
const flag = (n: number) => ({ '-1': 'Unknown', 0: 'None', 1: 'G', 2: 'B', 3: 'Y', 4: 'R' }[n] ?? String(n));
const skies = ['Clear', 'Light cloud', 'Overcast', 'Light rain', 'Heavy rain', 'Storm'];
const typeOf = (kind: RaceControlKind): string => typeof kind === 'string' ? kind : Object.keys(kind)[0]!;
const selected = (settings: Record<string, unknown>): readonly string[] => Array.isArray(settings.entryTypes) ? settings.entryTypes.filter((v): v is string => typeof v === 'string') : types;
const words = (name: string) => name.replace(/([a-z])([A-Z])/g, '$1 $2');
function gameText(event: Event): string {
  const details = event.details;
  if (typeof details === 'string') return details === 'Unknown' ? `Game event ${String.fromCharCode(...event.code)}` : words(details);
  if ('Penalty' in details) return `Penalty: ${details.Penalty.time} s, type ${details.Penalty.penalty_type}, infringement ${details.Penalty.infringement_type}`;
  if ('FastestLap' in details) return `Fastest lap: ${details.FastestLap.lap_time.toFixed(3)} s`;
  if ('SpeedTrap' in details) return 'Speed trap';
  if ('SafetyCar' in details) return `Safety car: type ${details.SafetyCar.safety_car_type}, event ${details.SafetyCar.event_type}`;
  return words(Object.keys(details)[0]!);
}
function text(kind: RaceControlKind): string {
  if (typeof kind === 'string') return labels[kind] ?? words(kind);
  if ('Game' in kind) return gameText(kind.Game);
  if ('FlagChanged' in kind) return `Flag ${flag(kind.FlagChanged.flag)}`;
  if ('MarshalFlagChanged' in kind) return `Marshal zone ${kind.MarshalFlagChanged.zone + 1}: ${flag(kind.MarshalFlagChanged.flag)}`;
  if ('SafetyCarChanged' in kind) return ['SC ended', 'SC', 'VSC', 'Formation'][kind.SafetyCarChanged.status] ?? `Safety car ${kind.SafetyCarChanged.status}`;
  if ('WeatherChanged' in kind) return `Weather: ${skies[kind.WeatherChanged.weather] ?? 'Unknown'}`;
  return `Flashback: ${kind.Flashback.from_time.toFixed(1)} → ${kind.Flashback.to_time.toFixed(1)} s`;
}

/** A short tag, a headline and a detail for one entry, as a broadcast event ticker words it. */
interface Headline {
  tag: string;
  tone: 'info' | 'good' | 'warn' | 'bad' | 'quiet';
  title: string;
  detail: string;
}
const FLAG_WORD: Record<number, string> = { 1: 'Green flag', 2: 'Blue flag', 3: 'Yellow flag', 4: 'Red flag' };
const lapTime = (seconds: number) => {
  const m = Math.floor(seconds / 60);
  const rest = seconds - m * 60;
  return m > 0 ? `${m}:${rest.toFixed(3).padStart(6, '0')}` : rest.toFixed(3);
};
function headline(kind: RaceControlKind, carNames: string, name: (index: number) => string): Headline {
  if (typeof kind === 'string') return { tag: 'PIT', tone: 'info', title: kind === 'PitEntry' ? 'Pit entry' : 'Pit exit', detail: carNames };
  if ('FlagChanged' in kind) {
    const f = kind.FlagChanged.flag;
    return { tag: 'FLAG', tone: f === 4 ? 'bad' : f === 3 ? 'warn' : f === 1 ? 'good' : 'info', title: FLAG_WORD[f] ?? `Flag ${flag(f)}`, detail: f === 0 ? 'Flag cleared' : carNames };
  }
  if ('MarshalFlagChanged' in kind) {
    const f = kind.MarshalFlagChanged.flag;
    return { tag: 'FLAG', tone: f === 4 ? 'bad' : f === 3 ? 'warn' : 'info', title: FLAG_WORD[f] ?? `Flag ${flag(f)}`, detail: `Marshal zone ${kind.MarshalFlagChanged.zone + 1}` };
  }
  if ('SafetyCarChanged' in kind) {
    const status = kind.SafetyCarChanged.status;
    return { tag: ['SC', 'SC', 'VSC', 'FORM'][status] ?? 'SC', tone: status === 0 ? 'good' : 'warn', title: text(kind), detail: status === 0 ? 'Racing resumes' : 'Deployed' };
  }
  if ('WeatherChanged' in kind) return { tag: 'WX', tone: 'info', title: 'Weather', detail: skies[kind.WeatherChanged.weather] ?? 'Unknown' };
  if ('Flashback' in kind) return { tag: 'FB', tone: 'quiet', title: 'Flashback', detail: text(kind).replace('Flashback: ', '') };
  const d = kind.Game.details;
  if (typeof d === 'string') {
    const tone = d === 'RedFlag' ? 'bad' : d === 'LightsOut' || d === 'SessionStarted' || d === 'DrsEnabled' ? 'good' : 'info';
    const tag = d === 'LightsOut' ? 'START' : d === 'RedFlag' || d === 'ChequeredFlag' ? 'FLAG' : d.startsWith('Drs') ? 'DRS' : 'RC';
    return { tag, tone, title: gameText(kind.Game), detail: carNames };
  }
  if ('Overtake' in d) return { tag: 'OVTK', tone: 'info', title: 'Overtake', detail: `${name(d.Overtake.overtaking_vehicle_idx)} passes ${name(d.Overtake.being_overtaken_vehicle_idx)}` };
  if ('FastestLap' in d) return { tag: 'FL', tone: 'good', title: 'Fastest lap', detail: `${name(d.FastestLap.vehicle_idx)} · ${lapTime(d.FastestLap.lap_time)}` };
  if ('Penalty' in d) return { tag: 'PEN', tone: 'warn', title: 'Penalty', detail: `${name(d.Penalty.vehicle_idx)}${d.Penalty.time > 0 && d.Penalty.time < 255 ? ` · +${d.Penalty.time} s` : ''}` };
  if ('Retirement' in d) return { tag: 'DNF', tone: 'bad', title: 'Retirement', detail: name(d.Retirement.vehicle_idx) };
  if ('Collision' in d) return { tag: 'COL', tone: 'warn', title: 'Collision', detail: `${name(d.Collision.vehicle1_idx)} & ${name(d.Collision.vehicle2_idx)}` };
  if ('RaceWinner' in d) return { tag: 'WIN', tone: 'good', title: 'Race winner', detail: name(d.RaceWinner.vehicle_idx) };
  if ('SpeedTrap' in d) return { tag: 'TRAP', tone: 'info', title: 'Speed trap', detail: `${name(d.SpeedTrap.vehicle_idx)} · ${Math.round(d.SpeedTrap.speed)} km/h` };
  if ('DriveThroughServed' in d) return { tag: 'PEN', tone: 'info', title: 'Drive-through served', detail: name(d.DriveThroughServed.vehicle_idx) };
  if ('StopGoServed' in d) return { tag: 'PEN', tone: 'info', title: 'Stop-go served', detail: name(d.StopGoServed.vehicle_idx) };
  if ('SafetyCar' in d) return { tag: 'SC', tone: 'warn', title: 'Safety car', detail: carNames };
  if ('StartLights' in d) return { tag: 'START', tone: 'warn', title: 'Start lights', detail: `${d.StartLights.num_lights} lights` };
  return { tag: 'RC', tone: 'info', title: gameText(kind.Game), detail: carNames };
}
const clock = (seconds: number) => {
  const t = Math.max(0, Math.floor(seconds));
  return `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(t % 60).padStart(2, '0')}`;
};
/** Track status first in the strip, in words: red flag, safety car or track clear. */
function statusOf(session: Parameters<typeof raceStatus>[0], history: readonly WireHistoryEntry[]): { word: string; tone: Headline['tone'] } {
  if (!session) return { word: 'No session', tone: 'quiet' };
  const band = raceStatus(session, history);
  return { word: band || 'Track clear', tone: band === 'Red flag' ? 'bad' : band ? 'warn' : 'good' };
}

type RaceControlHistory = Extract<WireHistoryEntry, { kind: 'raceControl' }>;

/**
 * One-line form for a wide, short tile: the panel's name, the track status, then the newest
 * entries left to right, as a broadcast race-control ticker.
 */
function Strip({ entries, participants, session, history, connected }: {
  entries: readonly RaceControlHistory[];
  participants: Participant[];
  session: Parameters<typeof raceStatus>[0];
  history: readonly WireHistoryEntry[];
  connected: boolean;
}) {
  const name = (index: number) => {
    const p = participants.find((c) => c.index === index);
    return p ? carLabel(p) : `Car ${index + 1}`;
  };
  const status = statusOf(session, history);
  const newest = entries.slice(-6).reverse();
  return (
    <section className="pw-rc-strip" aria-label="Race-control feed">
      <div className="pw-rc-head">
        <b>Race control</b>
        <span>Live event stream</span>
      </div>
      <span className="pw-rc-status" data-tone={status.tone} data-testid="race-control-status">{status.word}</span>
      <ol className="pw-rc-ticker" aria-label="Race-control entries">
        {newest.map((e) => {
          const h = headline(e.body.kind, e.body.cars.map(name).join(', '), name);
          return (
            <li key={e.id} data-rewound={e.body.rewound}>
              <span className="pw-rc-tag" data-tone={h.tone}>{h.tag}</span>
              <b>{h.title}</b>
              {h.detail ? <span className="pw-rc-detail">{h.detail}</span> : null}
              <span className="pw-rc-time pw-num">
                {e.body.lap_number === null ? '' : `L${e.body.lap_number} · `}{clock(e.body.session_time)}{e.body.rewound ? ' · Rewound' : ''}
              </span>
            </li>
          );
        })}
      </ol>
      {newest.length === 0 ? (
        <span className="pw-rc-empty" role="status">{connected ? 'No race-control events yet' : 'Collector unreachable'}</span>
      ) : null}
    </section>
  );
}

/** Below this height the feed is a one-line strip rather than a list. */
const STRIP_BELOW = 140;

export default defineTileModule({
  declaration: {
    type: 'race-control-feed', name: 'Race-control feed', binding: 'session-wide', rate: 'slow',
    minSize: { w: 4, h: 2 }, defaultSize: { w: 6, h: 6 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 240, minHeight: 184 }, { density: 'full', minWidth: 400, minHeight: 304 }],
    reads: [{ source: 'history', kinds: ['raceControl'] }, { source: 'state', keys: ['cars.participant'] }],
  },
  Component: function RaceControl({ settings, density, height }: TileProps) {
    const snapshot = useSlow(s => s);
    const participants = participantsOf(snapshot.state);
    const allowed = selected(settings);
    const entries = snapshot.history.filter(e => e.kind === 'raceControl').filter(e => allowed.includes(typeOf(e.body.kind)));
    if (height < STRIP_BELOW) {
      const session = snapshot.state?.session && 'value' in snapshot.state.session ? snapshot.state.session.value : null;
      return <Strip entries={entries as RaceControlHistory[]} participants={participants} session={session} history={snapshot.history} connected={snapshot.connection === 'open'} />;
    }
    const visible = entries.slice(-Math.max(1, Math.floor((height - 24) / (density === 'compact' ? 44 : 64)))).reverse();
    return <section className="pw-slow-tile" aria-label="Race-control feed">
      <span className="pw-label">Race control</span>
      <ol className="pw-race-feed" aria-label="Race-control entries">{visible.map(e => {
        const names = e.body.cars.map(index => { const p = participants.find(p => p.index === index); return p ? carLabel(p) : `Car ${index + 1}`; }).join(', ');
        return <li key={e.id} data-rewound={e.body.rewound}>
          <span className="pw-label">{e.body.session_time.toFixed(1)} s{e.body.lap_number === null ? '' : ` · Lap ${e.body.lap_number}`}{e.body.rewound ? ' · Rewound' : ''}{snapshot.connection !== 'open' ? ' · Stale' : ''}</span>
          <div>{text(e.body.kind)}{names ? ` · ${names}` : ''}</div>
          {density === 'full' ? <span className="pw-label">{e.body.source}</span> : null}
        </li>;
      })}</ol>
      {visible.length === 0 ? <p role="status">{snapshot.connection !== 'open' ? 'Collector unreachable' : allowed.length === 0 ? 'No entry types selected' : 'No race-control entries'}</p> : null}
    </section>;
  },
  Settings: function RaceControlSettings({ settings, onChange }: TileSettingsProps) {
    const entries = selected(settings);
    return <fieldset className="pw-race-filter"><legend>Entry types</legend>{types.map(type => <label key={type}>
      <input type="checkbox" checked={entries.includes(type)} onChange={e => onChange({ ...settings, entryTypes: e.target.checked ? [...entries, type] : entries.filter(t => t !== type) })} />
      {labels[type]}
    </label>)}</fieldset>;
  },
});
