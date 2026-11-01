import type { SessionState } from '../generated/wire/SessionState';
import type { WireHistoryEntry } from '../runtime/wire';

const SAFETY_CAR = ['', 'Safety car', 'Virtual safety car', 'Formation lap'];
const safetyCarBand = (session: SessionState | null | undefined): string => SAFETY_CAR[session?.safetyCarStatus ?? 0] ?? '';

/**
 * The race status band, empty while nothing is declared (spec section 11, Race status).
 * Event chronology decides when a red flag is superseded by a restart or lights out.
 */
export function raceStatus(session: SessionState | null | undefined, history: readonly WireHistoryEntry[]): string {
  let band = safetyCarBand(session);
  for (const entry of history) {
    if (entry.kind !== 'raceControl' || entry.body.rewound) continue;
    const kind = entry.body.kind;
    if (typeof kind === 'object' && 'Game' in kind) {
      const detail = kind.Game.details;
      if (detail === 'RedFlag') band = 'Red flag';
      if (detail === 'LightsOut' || detail === 'SessionStarted') band = safetyCarBand(session);
    }
  }
  return band;
}
