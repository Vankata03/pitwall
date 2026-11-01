import { expect, test } from 'vitest';
import type { SessionState } from '../generated/wire/SessionState';
import type { WireHistoryEntry } from '../runtime/wire';
import { sessionName, trackName } from './names';
import { raceStatus } from './race-status';

const session = (safetyCarStatus: number) => ({ safetyCarStatus }) as SessionState;
const game = (details: string, rewound = false) =>
  ({ kind: 'raceControl', body: { rewound, kind: { Game: { details } } } }) as unknown as WireHistoryEntry;

test('the status is the safety car state, empty while nothing is declared', () => {
  expect(raceStatus(session(0), [])).toBe('');
  expect(raceStatus(session(1), [])).toBe('Safety car');
  expect(raceStatus(session(2), [])).toBe('Virtual safety car');
});

test('a red flag holds until lights out, and a rewound one never counted', () => {
  expect(raceStatus(session(0), [game('RedFlag')])).toBe('Red flag');
  expect(raceStatus(session(0), [game('RedFlag'), game('LightsOut')])).toBe('');
  expect(raceStatus(session(0), [game('RedFlag', true)])).toBe('');
});

test('names are the appendix place and session names; unlisted IDs have none', () => {
  expect(trackName(10)).toBe('Spa');
  expect(sessionName(15)).toBe('Race');
  expect(trackName(1)).toBeUndefined();
  expect(sessionName(undefined)).toBeUndefined();
});
