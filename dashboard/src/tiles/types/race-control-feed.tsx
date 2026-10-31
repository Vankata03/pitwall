import type { Event } from '../../generated/wire/Event';
import type { RaceControlKind } from '../../generated/wire/RaceControlKind';
import { useSlow } from '../../runtime/react';
import { carLabel, participantsOf } from '../../session/cars';
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

export default defineTileModule({
  declaration: {
    type: 'race-control-feed', name: 'Race-control feed', binding: 'session-wide', rate: 'slow',
    minSize: { w: 4, h: 3 }, defaultSize: { w: 6, h: 6 },
    densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 240, minHeight: 184 }, { density: 'full', minWidth: 400, minHeight: 304 }],
    reads: [{ source: 'history', kinds: ['raceControl'] }, { source: 'state', keys: ['cars.participant'] }],
  },
  Component: function RaceControl({ settings, density, height }: TileProps) {
    const snapshot = useSlow(s => s);
    const participants = participantsOf(snapshot.state);
    const allowed = selected(settings);
    const entries = snapshot.history.filter(e => e.kind === 'raceControl').filter(e => allowed.includes(typeOf(e.body.kind)));
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
