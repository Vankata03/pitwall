import { FRAME_FIELD_GROUP, type FrameField, type ReadoutGroup } from './wire';

/** `follows-car` tiles are set to Focus (default), Pinned driver, Car ahead or Car behind. */
export type CarBinding = 'follows-car' | 'session-wide';
export type CarTarget = 'focus' | 'pinned' | 'car-ahead' | 'car-behind';

export type Density = 'compact' | 'regular' | 'full';
const DENSITY_ORDER: readonly Density[] = ['compact', 'regular', 'full'];

/** Grid cells. */
export interface TileSize {
  w: number;
  h: number;
}

/** A density applies once the tile is at least this many CSS pixels in both directions. */
export interface DensityRule {
  density: Density;
  minWidth: number;
  minHeight: number;
}

/** What a tile reads. Drives the dirty check, missing-data states and picker hints. */
export type DataRead =
  | { source: 'frame'; fields: readonly FrameField[] }
  | { source: 'state'; keys: readonly string[] }
  | { source: 'history'; kinds: readonly string[] };

/** `frame` tiles run at 60 Hz through `draw()`; `slow` tiles at 4 to 10 Hz through React. */
export type RateClass = 'frame' | 'slow';

export interface TileDeclaration {
  /** Catalogue key; a layout may hold several tiles of one type. */
  type: string;
  name: string;
  binding: CarBinding;
  rate: RateClass;
  minSize: TileSize;
  defaultSize: TileSize;
  /** Two or three, ascending from compact. No maximum size, and content is never scaled with CSS. */
  densities: readonly DensityRule[];
  reads: readonly DataRead[];
}

/** Validate and return a tile declaration; throws on a malformed one so mistakes surface at load. */
export function defineTile<T extends TileDeclaration>(decl: T): T {
  const ds = decl.densities;
  const ok =
    ds.length >= 2 &&
    ds.length <= 3 &&
    ds.every((d, i) => d.density === DENSITY_ORDER[i]);
  if (!ok) {
    throw new Error(
      `tile ${decl.type}: densities must be 2 or 3 entries in order compact, regular, full`,
    );
  }
  for (let i = 1; i < ds.length; i++) {
    if (ds[i]!.minWidth < ds[i - 1]!.minWidth || ds[i]!.minHeight < ds[i - 1]!.minHeight) {
      throw new Error(`tile ${decl.type}: densities must not shrink as they grow`);
    }
  }
  if (decl.defaultSize.w < decl.minSize.w || decl.defaultSize.h < decl.minSize.h) {
    throw new Error(`tile ${decl.type}: default size is below the minimum size`);
  }
  if (decl.rate === 'frame' && frameFieldsOf(decl).length === 0) {
    throw new Error(`tile ${decl.type}: a frame tile must read at least one frame field`);
  }
  return decl;
}

/** Density for the tile's actual rendered size. */
export function pickDensity(decl: TileDeclaration, width: number, height: number): Density {
  let chosen = decl.densities[0]!.density;
  for (const d of decl.densities) {
    if (width >= d.minWidth && height >= d.minHeight) chosen = d.density;
  }
  return chosen;
}

export function frameFieldsOf(decl: TileDeclaration): FrameField[] {
  const out: FrameField[] = [];
  for (const r of decl.reads) {
    if (r.source === 'frame') for (const f of r.fields) if (!out.includes(f)) out.push(f);
  }
  return out;
}

/** Readout groups behind the tile's frame fields, in first-use order. */
export function frameGroupsOf(decl: TileDeclaration): ReadoutGroup[] {
  const out: ReadoutGroup[] = [];
  for (const f of frameFieldsOf(decl)) {
    const g = FRAME_FIELD_GROUP[f];
    if (!out.includes(g)) out.push(g);
  }
  return out;
}
