import { describe, expect, test } from 'vitest';
import {
  addTile,
  arrangementOf,
  newTileId,
  placeBelow,
  removeTile,
  replaceArrangement,
  setTileSettings,
  toSavedBody,
  type Layout,
  type Tile,
} from './model';

const tile = (i: string, type: string, x: number, y: number, w: number, h: number, extra: object = {}): Tile => ({
  i,
  type,
  x,
  y,
  w,
  h,
  settings: {},
  ...extra,
});

const served = (overrides: Partial<Layout> = {}): Layout => ({
  schemaVersion: 1,
  name: 'Mine',
  preset: false,
  generated: ['tablet', 'phone'],
  arrangements: {
    wide: [tile('notice', 'notice', 0, 0, 24, 1)],
    tablet: [tile('notice', 'notice', 0, 0, 12, 1)],
    phone: [tile('notice', 'notice', 0, 0, 4, 1)],
  },
  ...overrides,
});

describe('arrangementOf', () => {
  test('returns the tiles of one screen class', () => {
    expect(arrangementOf(served(), 'tablet')).toEqual([tile('notice', 'notice', 0, 0, 12, 1)]);
  });
});

describe('replaceArrangement', () => {
  test('editing a generated class makes it the layout’s own', () => {
    const next = replaceArrangement(served(), 'tablet', [tile('notice', 'notice', 0, 2, 12, 1)]);
    expect(next.generated).toEqual(['phone']);
    expect(arrangementOf(next, 'tablet')[0]!.y).toBe(2);
  });

  test('editing wide leaves the generated classes generated', () => {
    const next = replaceArrangement(served(), 'wide', [tile('notice', 'notice', 0, 5, 24, 1)]);
    expect(next.generated).toEqual(['tablet', 'phone']);
  });

  test('does not change the layout it was given', () => {
    const before = served();
    const snapshot = JSON.stringify(before);
    replaceArrangement(before, 'phone', []);
    expect(JSON.stringify(before)).toBe(snapshot);
  });
});

describe('newTileId', () => {
  test('is the kebab-case type when free, then numbered', () => {
    expect(newTileId('lap-timing', [])).toBe('lap-timing');
    expect(newTileId('lap-timing', ['lap-timing'])).toBe('lap-timing-2');
    expect(newTileId('lap-timing', ['lap-timing', 'lap-timing-2'])).toBe('lap-timing-3');
  });
});

describe('addTile', () => {
  test('places the tile under everything else at its default size', () => {
    const tiles = [tile('a', 'a', 0, 0, 6, 4), tile('b', 'b', 6, 0, 6, 7)];
    const { tiles: next, added } = addTile(tiles, 'lap-timing', { w: 6, h: 4 }, 24);
    expect(added).toMatchObject({ i: 'lap-timing', type: 'lap-timing', x: 0, y: 7, w: 6, h: 4, settings: {} });
    expect(next).toHaveLength(3);
  });

  test('clamps the width to the class’s columns', () => {
    const { added } = addTile([], 'x', { w: 8, h: 2 }, 4);
    expect(added.w).toBe(4);
  });

  test('a second tile of a type gets its own id and settings', () => {
    const first = addTile([], 'weather', { w: 6, h: 2 }, 24).tiles;
    const { added } = addTile(first, 'weather', { w: 6, h: 2 }, 24);
    expect(added.i).toBe('weather-2');
  });
});

describe('placeBelow', () => {
  test('returns the first free row under the lowest tile', () => {
    expect(placeBelow([])).toBe(0);
    expect(placeBelow([tile('a', 'a', 0, 3, 2, 4), tile('b', 'b', 2, 0, 2, 2)])).toBe(7);
  });
});

describe('removeTile and setTileSettings', () => {
  const tiles = [tile('a', 'a', 0, 0, 2, 2, { mystery: 1 }), tile('b', 'b', 2, 0, 2, 2)];

  test('removeTile drops one tile by id', () => {
    expect(removeTile(tiles, 'a').map((t) => t.i)).toEqual(['b']);
  });

  test('setTileSettings replaces that tile’s settings and keeps everything else on it', () => {
    const next = setTileSettings(tiles, 'a', { car: { target: 'focus' }, shown: ['x'] });
    expect(next[0]).toEqual({ ...tiles[0]!, settings: { car: { target: 'focus' }, shown: ['x'] } });
    expect(next[1]).toBe(tiles[1]);
  });
});

describe('toSavedBody', () => {
  test('sends the whole layout with the classes still generated, and no preset flag', () => {
    const body = toSavedBody(served(), 'Mine');
    expect(body).toMatchObject({ schemaVersion: 1, name: 'Mine', generated: ['tablet', 'phone'] });
    expect(body).not.toHaveProperty('preset');
  });

  test('renames the body for a copy', () => {
    expect(toSavedBody(served({ name: 'Race', preset: true }), 'Race copy').name).toBe('Race copy');
  });

  test('keeps unknown fields on tiles and on the layout', () => {
    const layout = served({ future: { a: 1 } } as Partial<Layout>);
    layout.arrangements.wide = [tile('n', 'mystery-type', 0, 0, 2, 2, { colour: 'red', settings: { z: 1 } })];
    const body = toSavedBody(layout, 'Mine') as Record<string, unknown>;
    expect(body.future).toEqual({ a: 1 });
    expect((body.arrangements as Layout['arrangements']).wide[0]).toMatchObject({
      type: 'mystery-type',
      colour: 'red',
      settings: { z: 1 },
    });
  });
});
