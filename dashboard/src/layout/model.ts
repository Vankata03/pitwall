import type { TileSize } from '../runtime/tile';
import type { ScreenClass } from './screen';

/** One tile instance: `{i, type, x, y, w, h, settings}`. Unknown fields ride along untouched. */
export interface Tile {
  i: string;
  type: string;
  x: number;
  y: number;
  w: number;
  h: number;
  settings: Record<string, unknown>;
  [extra: string]: unknown;
}

/** A layout as `GET /api/layouts/{name}` serves it: every class filled in. */
export interface Layout {
  schemaVersion: number;
  name: string;
  /** Presets are read-only. */
  preset: boolean;
  /** Classes nobody has arranged, derived from wide until edited. */
  generated: readonly ScreenClass[];
  arrangements: Record<ScreenClass, Tile[]>;
  [extra: string]: unknown;
}

export function arrangementOf(layout: Layout, screenClass: ScreenClass): Tile[] {
  return layout.arrangements[screenClass];
}

/**
 * Replace one class's tiles. A class edited this way is no longer `generated`, which is how
 * the server learns to keep it. Other classes are untouched.
 */
export function replaceArrangement(layout: Layout, screenClass: ScreenClass, tiles: Tile[]): Layout {
  return {
    ...layout,
    generated: layout.generated.filter((c) => c !== screenClass),
    arrangements: { ...layout.arrangements, [screenClass]: tiles },
  };
}

/** First row under every tile. */
export function placeBelow(tiles: readonly Tile[]): number {
  return tiles.reduce((bottom, t) => Math.max(bottom, t.y + t.h), 0);
}

/** A kebab-case id: the type itself while free, then `type-2`, `type-3`. */
export function newTileId(type: string, taken: readonly string[]): string {
  if (!taken.includes(type)) return type;
  let n = 2;
  while (taken.includes(`${type}-${n}`)) n++;
  return `${type}-${n}`;
}

/** Add a tile of `type` at its default size under the others. */
export function addTile(
  tiles: readonly Tile[],
  type: string,
  size: TileSize,
  columns: number,
): { tiles: Tile[]; added: Tile } {
  const added: Tile = {
    i: newTileId(type, tiles.map((t) => t.i)),
    type,
    x: 0,
    y: placeBelow(tiles),
    w: Math.min(size.w, columns),
    h: size.h,
    settings: {},
  };
  return { tiles: [...tiles, added], added };
}

export function removeTile(tiles: readonly Tile[], id: string): Tile[] {
  return tiles.filter((t) => t.i !== id);
}

export function setTileSettings(tiles: readonly Tile[], id: string, settings: Record<string, unknown>): Tile[] {
  return tiles.map((t) => (t.i === id ? { ...t, settings } : t));
}

/**
 * The body for `PUT /api/layouts/{name}`: the layout with every class filled in and
 * `generated` naming the ones that stay unarranged. `preset` is server-owned.
 */
export function toSavedBody(layout: Layout, name: string): Omit<Layout, 'preset'> {
  const { preset: _preset, ...rest } = layout;
  return { ...rest, name };
}
