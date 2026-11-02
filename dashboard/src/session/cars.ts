/**
 * Cars as the dashboard chrome needs them: who is in the session, which car is the focus,
 * and which car a tile follows. Reads State through plain structural checks so it works
 * against the wire boundary today and the generated types later (see `reading State`).
 *
 * Wire shape this file reads (the generated `State`):
 * `state.playerCar: number | null`, `state.cars[i]: { participant: Readout<ParticipantData>,
 * retired: boolean, lap: Readout<{ car_position }> } | null`, and
 * `state.context.revisions.layouts: number`. `ParticipantData` keeps the game's snake_case
 * names and `name` is the game's 32 NUL-padded bytes.
 */

/**
 * How a car is recognised across sessions and devices (spec section 5): AI and offline
 * humans by `driverId`, online humans by name, because `networkId` is session-scoped.
 */
export type CarIdentity = { kind: 'driver'; driverId: number } | { kind: 'name'; name: string };

export interface Participant {
  /** Vehicle index: identifies the car within the session. */
  index: number;
  name: string;
  raceNumber: number;
  /** Whether the name can be displayed. Hidden online-human names may contain platform IDs. */
  showOnlineNames: boolean;
  identity: CarIdentity;
  retired: boolean;
  /** Race position, when the lap data has it. */
  position: number | undefined;
}

type Obj = Record<string, unknown>;
const isObj = (x: unknown): x is Obj => typeof x === 'object' && x !== null && !Array.isArray(x);
const num = (x: unknown): number | undefined => (typeof x === 'number' && Number.isFinite(x) ? x : undefined);

/** The game sends 255 as the network id of a car that is not an online human. */
const NOT_NETWORKED = 255;

function decodeName(raw: unknown): string {
  if (typeof raw === 'string') return raw;
  if (!Array.isArray(raw)) return '';
  const end = raw.indexOf(0);
  const bytes = Uint8Array.from(end === -1 ? raw : raw.slice(0, end));
  return new TextDecoder().decode(bytes);
}

/** Identity of a car from its `ParticipantData`. */
export function identityOf(data: Obj): CarIdentity {
  const online = data.ai_controlled === 0 && (num(data.network_id) ?? NOT_NETWORKED) !== NOT_NETWORKED;
  return online
    ? { kind: 'name', name: decodeName(data.name) }
    : { kind: 'driver', driverId: num(data.driver_id) ?? 0 };
}

/** Everyone in the session with a participant record, in vehicle-index order. */
export function participantsOf(state: Obj | null): Participant[] {
  const cars = state?.cars;
  if (!Array.isArray(cars)) return [];
  const out: Participant[] = [];
  cars.forEach((car, index) => {
    if (!isObj(car) || !isObj(car.participant)) return;
    const { state: readout, value } = car.participant;
    if ((readout !== 'live' && readout !== 'stale') || !isObj(value)) return;
    const lap = isObj(car.lap) && isObj(car.lap.value) ? car.lap.value : undefined;
    out.push({
      index,
      name: decodeName(value.name),
      raceNumber: num(value.race_number) ?? 0,
      showOnlineNames: value.ai_controlled === 1 || value.show_online_names !== 0,
      identity: identityOf(value),
      retired: car.retired === true,
      position: num(lap?.car_position),
    });
  });
  return out;
}

export function playerCarOf(state: Obj | null): number | null {
  return num(state?.playerCar) ?? null;
}

/** Revision of the layout library; advances whenever any screen changes it. */
export function layoutsRevisionOf(state: Obj | null): number | undefined {
  const context = state?.context;
  if (!isObj(context) || !isObj(context.revisions)) return undefined;
  return num(context.revisions.layouts);
}

/** Shown name: `m_name` as received, or `#<race number>` for a human who hides it. */
export function carLabel(p: Participant): string {
  return p.showOnlineNames && p.name.trim() ? p.name : `#${p.raceNumber}`;
}

export function identityKey(identity: CarIdentity): string {
  return JSON.stringify(identity);
}

export function parseIdentity(text: string | null): CarIdentity | null {
  if (text === null) return null;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isObj(value)) return null;
  if (value.kind === 'driver' && num(value.driverId) !== undefined) {
    return { kind: 'driver', driverId: value.driverId as number };
  }
  if (value.kind === 'name' && typeof value.name === 'string') return { kind: 'name', name: value.name };
  return null;
}

/** Where a binding landed. `retired` keeps the car so the tile can say so; no silent fallback. */
export type ResolvedCar =
  | { status: 'ok'; index: number }
  | { status: 'retired'; index: number }
  | { status: 'not-in-session' }
  /** Car ahead or behind with nobody there. */
  | { status: 'none' };

export function resolveIdentity(identity: CarIdentity, participants: readonly Participant[]): ResolvedCar {
  const found = participants.find((p) => identityKey(p.identity) === identityKey(identity));
  if (!found) return { status: 'not-in-session' };
  return { status: found.retired ? 'retired' : 'ok', index: found.index };
}

export type CarTarget = 'focus' | 'pinned' | 'car-ahead' | 'car-behind';

/** A tile's car setting, stored in its settings as `settings.car`. */
export interface CarBindingSetting {
  target: CarTarget;
  pin?: CarIdentity;
}

export interface BindingContext {
  participants: readonly Participant[];
  playerCar: number | null;
  /** This device's chosen focus car; null follows the player car. */
  focus: CarIdentity | null;
}

/** The car the device focuses on. */
export function resolveFocus(ctx: BindingContext): ResolvedCar {
  if (ctx.focus) return resolveIdentity(ctx.focus, ctx.participants);
  if (ctx.playerCar === null) return { status: 'not-in-session' };
  const player = ctx.participants.find((p) => p.index === ctx.playerCar);
  return player ? { status: player.retired ? 'retired' : 'ok', index: player.index } : { status: 'not-in-session' };
}

export function resolveBinding(binding: CarBindingSetting, ctx: BindingContext): ResolvedCar {
  switch (binding.target) {
    case 'pinned':
      return binding.pin ? resolveIdentity(binding.pin, ctx.participants) : { status: 'not-in-session' };
    case 'focus':
      return resolveFocus(ctx);
    case 'car-ahead':
    case 'car-behind': {
      const focus = resolveFocus(ctx);
      if (focus.status === 'not-in-session' || focus.status === 'none') return focus;
      const here = ctx.participants.find((p) => p.index === focus.index)?.position;
      if (here === undefined) return { status: 'none' };
      const wanted = binding.target === 'car-ahead' ? here - 1 : here + 1;
      const next = ctx.participants.find((p) => p.position === wanted);
      return next ? { status: next.retired ? 'retired' : 'ok', index: next.index } : { status: 'none' };
    }
  }
}

/** A tile's car setting with the default, Focus. */
export function carSettingOf(settings: Record<string, unknown>): CarBindingSetting {
  const car = settings.car;
  if (isObj(car)) {
    const target = car.target;
    if (target === 'focus' || target === 'pinned' || target === 'car-ahead' || target === 'car-behind') {
      const pin = isObj(car.pin) ? parseIdentity(JSON.stringify(car.pin)) : null;
      return pin ? { target, pin } : { target };
    }
  }
  return { target: 'focus' };
}
