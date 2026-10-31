import type { ComponentType } from 'react';
import type { Units } from '../device/units';
import type { ResolvedCar } from '../session/cars';
import { defineTile, type Density, type TileDeclaration } from '../runtime/tile';

/** What the host gives a tile. Tiles never measure themselves or read device storage. */
export interface TileProps {
  /** The instance id in the layout; unique within an arrangement. */
  id: string;
  /** This instance's settings, as saved. Unknown keys must be left alone. */
  settings: Record<string, unknown>;
  /** Chosen from the tile's rendered size, by the declaration's density rules. */
  density: Density;
  /** Rendered size in CSS pixels. */
  width: number;
  height: number;
  /**
   * The car this tile follows, resolved from its car setting; null for a session-wide tile.
   * The host shows "car not in session" in place of the tile when the car is missing, so a
   * tile sees `ok` or `retired` (and `none` is shown by the host too).
   */
  car: ResolvedCar | null;
  /** This device's unit choices; convert with `device/units`. */
  units: Units;
  /** Edit mode: the tile stays live under its drag handles. */
  editing: boolean;
}

export interface TileSettingsProps {
  settings: Record<string, unknown>;
  /** Replace the instance's settings. Spread `settings` first so unknown keys survive. */
  onChange(settings: Record<string, unknown>): void;
}

/** One tile type: its declaration and the component the host mounts. One per file. */
export interface TileModule {
  declaration: TileDeclaration;
  Component: ComponentType<TileProps>;
  /**
   * Tile-specific settings for the settings dialog. The car setting of a `follows-car` tile
   * is added by the host and needs no field here.
   */
  Settings?: ComponentType<TileSettingsProps>;
}

const KEBAB = /^[a-z0-9]+(-[a-z0-9]+)*$/;

/**
 * Validate and return a tile module; throws at load on a malformed one. The type doubles as
 * the default tile id in layouts, so it must be kebab-case.
 */
export function defineTileModule(module: TileModule): TileModule {
  const { type } = module.declaration;
  if (!KEBAB.test(type)) throw new Error(`tile type "${type}" must be kebab-case, like "lap-timing"`);
  defineTile(module.declaration);
  return module;
}
