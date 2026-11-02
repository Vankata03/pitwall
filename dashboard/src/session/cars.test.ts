import { describe, expect, test } from 'vitest';
import {
  carLabel,
  identityKey,
  identityOf,
  layoutsRevisionOf,
  parseIdentity,
  participantsOf,
  playerCarOf,
  resolveBinding,
  resolveIdentity,
  type Participant,
} from './cars';

const bytes = (text: string): number[] => {
  const out = [...new TextEncoder().encode(text)];
  while (out.length < 32) out.push(0);
  return out;
};

const participantData = (o: Record<string, unknown> = {}) => ({
  ai_controlled: 1,
  driver_id: 9,
  network_id: 255,
  team_id: 3,
  my_team: 0,
  race_number: 44,
  name: bytes('HAMILTON'),
  show_online_names: 1,
  ...o,
});

const car = (data: Record<string, unknown> | null, extra: Record<string, unknown> = {}) =>
  data === null
    ? { participant: { state: 'absent' }, retired: false, ...extra }
    : { participant: { state: 'live', value: participantData(data) }, retired: false, ...extra };

describe('reading State through the wire boundary', () => {
  const state = {
    playerCar: 2,
    cars: [car({ driver_id: 1, name: bytes('Hülkenberg') }), null, car({ driver_id: 5, name: bytes('Alonso') })],
    context: { revisions: { layouts: 7, tracks: 1 } },
  };

  test('participants carry index, name, race number and identity', () => {
    const list = participantsOf(state);
    expect(list.map((p) => [p.index, p.name, p.raceNumber])).toEqual([
      [0, 'Hülkenberg', 44],
      [2, 'Alonso', 44],
    ]);
    expect(list[0]!.identity).toEqual({ kind: 'driver', driverId: 1 });
  });

  test('the player car and layout revision come from State', () => {
    expect(playerCarOf(state)).toBe(2);
    expect(layoutsRevisionOf(state)).toBe(7);
  });

  test('missing or malformed State reads as nothing, never throws', () => {
    expect(participantsOf(null)).toEqual([]);
    expect(participantsOf({ cars: 'nope' })).toEqual([]);
    expect(playerCarOf({})).toBeNull();
    expect(layoutsRevisionOf({})).toBeUndefined();
    expect(layoutsRevisionOf({ context: { revisions: { layouts: 'x' } } })).toBeUndefined();
  });

  test('a stale participant still lists; restricted and absent do not', () => {
    const stale = { cars: [{ participant: { state: 'stale', value: participantData() }, retired: false }] };
    expect(participantsOf(stale)).toHaveLength(1);
    expect(participantsOf({ cars: [{ participant: { state: 'restricted' }, retired: false }] })).toEqual([]);
  });

  test('retired cars stay listed and are marked', () => {
    const list = participantsOf({ cars: [car({}, { retired: true })] });
    expect(list[0]!.retired).toBe(true);
  });
});

describe('identity: one rule for focus and pins (spec section 5)', () => {
  test('AI and offline humans are identified by driver id', () => {
    expect(identityOf(participantData({ ai_controlled: 1, driver_id: 12 }))).toEqual({ kind: 'driver', driverId: 12 });
    expect(identityOf(participantData({ ai_controlled: 0, network_id: 255, driver_id: 0 }))).toEqual({
      kind: 'driver',
      driverId: 0,
    });
  });

  test('online humans are identified by name, because the network id is session-scoped', () => {
    expect(identityOf(participantData({ ai_controlled: 0, network_id: 17, name: bytes('Max') }))).toEqual({
      kind: 'name',
      name: 'Max',
    });
  });

  test('a pin survives into another session by driver id', () => {
    const next = participantsOf({ cars: [car({ driver_id: 20 }), car({ driver_id: 9 })] });
    expect(resolveIdentity({ kind: 'driver', driverId: 9 }, next)).toEqual({ status: 'ok', index: 1 });
  });

  test('a pin absent from the session says so and is not replaced', () => {
    expect(resolveIdentity({ kind: 'driver', driverId: 99 }, participantsOf({ cars: [car({})] }))).toEqual({
      status: 'not-in-session',
    });
  });

  test('a retired car keeps the focus and reports retired, with no fallback', () => {
    const list = participantsOf({ cars: [car({ driver_id: 9 }, { retired: true })] });
    expect(resolveIdentity({ kind: 'driver', driverId: 9 }, list)).toEqual({ status: 'retired', index: 0 });
  });

  test('stores and parses round-trip, and rejects junk', () => {
    for (const id of [{ kind: 'driver', driverId: 4 }, { kind: 'name', name: 'Max' }] as const) {
      expect(parseIdentity(identityKey(id))).toEqual(id);
    }
    expect(parseIdentity('garbage')).toBeNull();
    expect(parseIdentity('{"kind":"driver","driverId":"x"}')).toBeNull();
    expect(parseIdentity(null)).toBeNull();
  });
});

describe('labels', () => {
  const p = (o: Partial<Participant>): Participant => ({
    index: 0,
    name: 'Max',
    raceNumber: 33,
    showOnlineNames: true,
    identity: { kind: 'name', name: 'Max' },
    retired: false,
    position: undefined,
    ...o,
  });

  test('name as received', () => {
    expect(carLabel(p({}))).toBe('Max');
  });

  test('AI names remain visible when the online-name flag is off', () => {
    const drivers = participantsOf({ cars: [car({ ai_controlled: 1, network_id: 65535, show_online_names: 0, name: bytes('PÉREZ') })] });
    expect(carLabel(drivers[0]!)).toBe('PÉREZ');
  });

  test('hidden online human names remain private and empty names fall back to race number', () => {
    const drivers = participantsOf({ cars: [car({ ai_controlled: 0, network_id: 17, show_online_names: 0, name: bytes('private-platform-id') })] });
    expect(carLabel(drivers[0]!)).toBe('#44');
    expect(carLabel(p({ name: '  ' }))).toBe('#33');
  });

  test('a human who hides their online name is shown by race number', () => {
    expect(carLabel(p({ showOnlineNames: false }))).toBe('#33');
  });
});

describe('binding a tile to a car', () => {
  const state = {
    playerCar: 1,
    cars: [
      { ...car({ driver_id: 1 }), lap: { state: 'live', value: { car_position: 1 } } },
      { ...car({ driver_id: 2 }), lap: { state: 'live', value: { car_position: 2 } } },
      { ...car({ driver_id: 3 }), lap: { state: 'live', value: { car_position: 3 } } },
    ],
  };
  const participants = participantsOf(state);
  const ctx = { participants, playerCar: 1, focus: null };

  test('Focus follows the player car by default', () => {
    expect(resolveBinding({ target: 'focus' }, ctx)).toEqual({ status: 'ok', index: 1 });
  });

  test('Focus follows a stored focus identity, even one absent from the session', () => {
    expect(resolveBinding({ target: 'focus' }, { ...ctx, focus: { kind: 'driver', driverId: 3 } })).toEqual({
      status: 'ok',
      index: 2,
    });
    expect(resolveBinding({ target: 'focus' }, { ...ctx, focus: { kind: 'driver', driverId: 77 } })).toEqual({
      status: 'not-in-session',
    });
  });

  test('no player car yet resolves to nothing, not to car 0', () => {
    expect(resolveBinding({ target: 'focus' }, { ...ctx, playerCar: null })).toEqual({ status: 'not-in-session' });
  });

  test('Pinned driver ignores the focus', () => {
    const binding = { target: 'pinned' as const, pin: { kind: 'driver' as const, driverId: 3 } };
    expect(resolveBinding(binding, ctx)).toEqual({ status: 'ok', index: 2 });
  });

  test('Pinned driver with no pin chosen yet is not in session', () => {
    expect(resolveBinding({ target: 'pinned' }, ctx)).toEqual({ status: 'not-in-session' });
  });

  test('Car ahead and Car behind are the neighbours in race position of the focus car', () => {
    expect(resolveBinding({ target: 'car-ahead' }, ctx)).toEqual({ status: 'ok', index: 0 });
    expect(resolveBinding({ target: 'car-behind' }, ctx)).toEqual({ status: 'ok', index: 2 });
  });

  test('the leader has no car ahead', () => {
    const leader = { ...ctx, focus: { kind: 'driver' as const, driverId: 1 } };
    expect(resolveBinding({ target: 'car-ahead' }, leader)).toEqual({ status: 'none' });
  });
});
