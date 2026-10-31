/** Test-only: an in-memory stand-in for the collector's layout routes. */
import { LayoutsApiError, type LayoutsApi } from './api';
import type { Layout, Tile } from './model';

export const tile = (i: string, type: string, x: number, y: number, w: number, h: number): Tile => ({
  i,
  type,
  x,
  y,
  w,
  h,
  settings: {},
});

export function layoutOf(name: string, preset = false, generated: Layout['generated'] = []): Layout {
  return {
    schemaVersion: 1,
    name,
    preset,
    generated,
    arrangements: {
      wide: [tile('notice', 'notice', 0, 0, 24, 1), tile('future-tile', 'future-tile', 0, 1, 6, 2)],
      tablet: [tile('notice', 'notice', 0, 0, 12, 1), tile('future-tile', 'future-tile', 0, 1, 6, 2)],
      phone: [tile('notice', 'notice', 0, 0, 4, 1), tile('future-tile', 'future-tile', 0, 1, 4, 2)],
    },
  };
}

const clone = <T,>(x: T): T => JSON.parse(JSON.stringify(x)) as T;

export interface FakeServer extends LayoutsApi {
  revision: number;
  layouts: Map<string, Layout>;
  /** Every mutating call, in order, as `METHOD name` strings. */
  log: string[];
  /** Fail the next mutation with this status. */
  failNext(status: number, message: string): void;
}

export function fakeServer(extra: Layout[] = []): FakeServer {
  const layouts = new Map<string, Layout>();
  for (const name of ['Pitwall Classic', 'Driver', 'Race']) layouts.set(name, layoutOf(name, true));
  for (const l of extra) layouts.set(l.name, clone(l));
  let failure: { status: number; message: string } | null = null;
  const server: FakeServer = {
    revision: 0,
    layouts,
    log: [],
    failNext: (status, message) => {
      failure = { status, message };
    },
    async list() {
      return {
        revision: server.revision,
        layouts: [...layouts.values()].map((l) => ({ name: l.name, preset: l.preset })),
      };
    },
    async get(name) {
      const found = layouts.get(name);
      return found ? clone(found) : null;
    },
    async save(name, body) {
      mutate(`PUT ${name}`);
      if (layouts.get(name)?.preset) throw new LayoutsApiError(403, 'presets are read-only');
      const saved = { ...clone(body as Layout), name, preset: false };
      layouts.set(name, saved);
      server.revision++;
      return clone(saved);
    },
    async duplicate(name, newName) {
      mutate(`POST duplicate ${name}`);
      const source = layouts.get(name);
      if (!source) throw new LayoutsApiError(404, 'no such layout');
      let copyName = newName ?? `${name} copy`;
      for (let n = 2; !newName && layouts.has(copyName); n++) copyName = `${name} copy ${n}`;
      if (layouts.has(copyName)) throw new LayoutsApiError(409, 'name already in use');
      const copy = { ...clone(source), name: copyName, preset: false };
      layouts.set(copyName, copy);
      server.revision++;
      return clone(copy);
    },
    async rename(name, newName) {
      mutate(`POST rename ${name}`);
      const source = layouts.get(name);
      if (!source) throw new LayoutsApiError(404, 'no such layout');
      if (source.preset) throw new LayoutsApiError(403, 'presets are read-only');
      if (layouts.has(newName)) throw new LayoutsApiError(409, 'name already in use');
      layouts.delete(name);
      layouts.set(newName, { ...source, name: newName });
      server.revision++;
      return clone(layouts.get(newName)!);
    },
    async remove(name) {
      mutate(`DELETE ${name}`);
      const source = layouts.get(name);
      if (!source) throw new LayoutsApiError(404, 'no such layout');
      if (source.preset) throw new LayoutsApiError(403, 'presets are read-only');
      layouts.delete(name);
      server.revision++;
    },
  };
  function mutate(entry: string): void {
    server.log.push(entry);
    if (failure) {
      const f = failure;
      failure = null;
      throw new LayoutsApiError(f.status, f.message);
    }
  }
  return server;
}
