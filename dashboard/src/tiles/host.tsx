import type { Units } from '../device/units';
import { pickDensity } from '../runtime/tile';
import { carSettingOf, type ResolvedCar } from '../session/cars';
import type { Tile } from '../layout/model';
import type { TileRegistry } from './registry';
import { UnknownTile } from './unknown';

export interface TileHostProps {
  tile: Tile;
  registry: TileRegistry;
  /** Rendered size in CSS pixels. */
  width: number;
  height: number;
  /** Null for a session-wide tile. */
  car: ResolvedCar | null;
  units: Units;
  editing: boolean;
}

/** Whole-tile message for a car that is not there; null when the tile can draw. */
export function missingCarMessage(car: ResolvedCar | null, settings: Record<string, unknown>): string | null {
  if (!car) return null;
  if (car.status === 'not-in-session') return 'Car not in session';
  if (car.status === 'none') {
    return carSettingOf(settings).target === 'car-ahead' ? 'No car ahead' : 'No car behind';
  }
  return null;
}

/**
 * Mounts one tile: chooses its density from its rendered size, stands in for a car that is
 * not in the session, and draws the placeholder for an unknown type. The tile keeps its
 * shape throughout, so a message never reflows the grid.
 */
export function TileHost({ tile, registry, width, height, car, units, editing }: TileHostProps) {
  const module = registry.get(tile.type);
  if (!module) return <UnknownTile type={tile.type} />;
  const missing = missingCarMessage(car, tile.settings);
  if (missing) {
    return (
      <div className="pw-tile-message" role="status">
        <span className="pw-label">{module.declaration.name}</span>
        <span className="pw-value">{missing}</span>
      </div>
    );
  }
  const { Component } = module;
  return (
    <Component
      id={tile.i}
      settings={tile.settings}
      density={pickDensity(module.declaration, width, height)}
      width={width}
      height={height}
      car={car}
      units={units}
      editing={editing}
    />
  );
}
