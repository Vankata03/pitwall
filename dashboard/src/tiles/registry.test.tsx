import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, test } from 'vitest';
import { defineTileModule, type TileModule, type TileProps } from './contract';
import { createRegistry, registry } from './registry';
import { SHORT_LINE } from '../shared/short-line';

const probe = (type: string): TileModule =>
  defineTileModule({
    declaration: {
      type,
      name: `Probe ${type}`,
      binding: 'session-wide',
      rate: 'slow',
      minSize: { w: 2, h: 1 },
      defaultSize: { w: 4, h: 2 },
      densities: [
        { density: 'compact', minWidth: 0, minHeight: 0 },
        { density: 'regular', minWidth: 200, minHeight: 100 },
      ],
      reads: [],
    },
    Component: () => <p>probe</p>,
  });

const props = (overrides: Partial<TileProps> = {}): TileProps => ({
  id: 'x',
  settings: {},
  density: 'compact',
  width: 100,
  height: 40,
  car: null,
  units: { speed: 'kmh', temperature: 'c', pressure: 'psi' },
  editing: false,
  ...overrides,
});

describe('tile registry: one tile is one file', () => {
  test('finds a tile by type', () => {
    const a = probe('lap-timing');
    const reg = createRegistry([a, probe('weather')]);
    expect(reg.get('lap-timing')).toBe(a);
    expect(reg.all().map((m) => m.declaration.type)).toEqual(['lap-timing', 'weather']);
  });

  test('an unknown type finds nothing, and the placeholder takes its place', () => {
    expect(createRegistry([]).get('warp-core')).toBeUndefined();
  });

  test('two tiles of one type are a mistake caught at load', () => {
    expect(() => createRegistry([probe('weather'), probe('weather')])).toThrow(/weather.*twice/);
  });

  test('a type must be kebab-case, as layout tile ids are', () => {
    for (const bad of ['Lap Timing', 'lap_timing', 'LapTiming', '', '-x', 'x-', 'a--b']) {
      expect(() => probe(bad), bad).toThrow(/kebab-case/);
    }
    expect(() => probe('g-force')).not.toThrow();
    expect(() => probe('sm')).not.toThrow();
  });

  test('the add-tile picker lists tiles by name', () => {
    const reg = createRegistry([probe('b-tile'), probe('a-tile')]);
    expect(reg.all().map((m) => m.declaration.name)).toEqual(['Probe a-tile', 'Probe b-tile']);
  });
});

describe('shipped registry', () => {
  test('holds the Notice tile, which carries the short line into Stream view', () => {
    const notice = registry.get('notice');
    expect(notice).toBeDefined();
    expect(notice!.declaration).toMatchObject({ binding: 'session-wide', minSize: { w: 4, h: 1 }, defaultSize: { w: 6, h: 1 } });
    const Notice = notice!.Component;
    const html = renderToStaticMarkup(<Notice {...props()} />);
    expect(html).toContain(SHORT_LINE);
    expect(html).toContain('href="/status#notice"');
  });
});
