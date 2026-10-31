import { identityKey, parseIdentity, type CarIdentity } from '../session/cars';
import { DEFAULT_UNITS, type Units } from './units';

export interface DeviceSettings {
  /** Name of the layout this device shows; null until it has chosen (then Pitwall Classic). */
  layout: string | null;
  /** Chosen focus car; null follows the player car. */
  focus: CarIdentity | null;
  units: Units;
}

export type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

const KEY = { layout: 'pitwall.layout', focus: 'pitwall.focus', units: 'pitwall.units' } as const;

/** Layout, focus car and units for this browser, in browser storage. */
export interface Device {
  readonly storage: StorageLike;
  getSnapshot(): DeviceSettings;
  subscribe(listener: () => void): () => void;
  setLayout(name: string): void;
  /** Null is Follow player. */
  setFocus(identity: CarIdentity | null): void;
  setUnits(units: Units): void;
}

/** In-memory storage, for tests and for browsers that block theirs. */
export function memoryStorage(initial: Record<string, string> = {}): StorageLike {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => map.get(k) ?? null,
    setItem: (k, v) => void map.set(k, v),
    removeItem: (k) => void map.delete(k),
  };
}

function safely<T>(fn: () => T, fallback: T): T {
  try {
    return fn();
  } catch {
    return fallback;
  }
}

function readUnits(text: string | null): Units {
  let raw: Record<string, unknown> = {};
  try {
    const parsed: unknown = text === null ? {} : JSON.parse(text);
    if (typeof parsed === 'object' && parsed !== null) raw = parsed as Record<string, unknown>;
  } catch {
    // Corrupt value: use the defaults.
  }
  const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
    allowed.includes(value as T) ? (value as T) : fallback;
  return {
    speed: pick(raw.speed, ['kmh', 'mph'], DEFAULT_UNITS.speed),
    temperature: pick(raw.temperature, ['c', 'f'], DEFAULT_UNITS.temperature),
    pressure: pick(raw.pressure, ['psi', 'bar'], DEFAULT_UNITS.pressure),
  };
}

export function createDevice(storage: StorageLike): Device {
  const read = (): DeviceSettings => ({
    layout: safely(() => storage.getItem(KEY.layout), null),
    focus: parseIdentity(safely(() => storage.getItem(KEY.focus), null)),
    units: readUnits(safely(() => storage.getItem(KEY.units), null)),
  });
  let snapshot = read();
  const listeners = new Set<() => void>();

  function commit(next: DeviceSettings, persist: () => void): void {
    safely(persist, undefined);
    snapshot = next;
    for (const l of [...listeners]) l();
  }

  return {
    storage,
    getSnapshot: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    setLayout(name) {
      if (snapshot.layout === name) return;
      commit({ ...snapshot, layout: name }, () => storage.setItem(KEY.layout, name));
    },
    setFocus(identity) {
      const same =
        identity === null || snapshot.focus === null
          ? identity === snapshot.focus
          : identityKey(identity) === identityKey(snapshot.focus);
      if (same) return;
      commit({ ...snapshot, focus: identity }, () =>
        identity === null ? storage.removeItem(KEY.focus) : storage.setItem(KEY.focus, identityKey(identity)),
      );
    },
    setUnits(units) {
      const u = snapshot.units;
      if (u.speed === units.speed && u.temperature === units.temperature && u.pressure === units.pressure) return;
      commit({ ...snapshot, units: { ...units } }, () => storage.setItem(KEY.units, JSON.stringify(units)));
    },
  };
}

/** `?layout=<name>` opens a fixed layout without touching device memory. */
export function layoutNameFromUrl(search: string): string | null {
  const name = new URLSearchParams(search).get('layout');
  return name ? name : null;
}
