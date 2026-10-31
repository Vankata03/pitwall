import type { TileModule } from './contract';

export interface TileRegistry {
  /** Undefined for a type this build does not know: the host draws the placeholder. */
  get(type: string): TileModule | undefined;
  /** Every tile, by name, for the add-tile picker. */
  all(): readonly TileModule[];
}

export function createRegistry(modules: readonly TileModule[]): TileRegistry {
  const byType = new Map<string, TileModule>();
  for (const m of modules) {
    const { type } = m.declaration;
    if (byType.has(type)) throw new Error(`tile type "${type}" is registered twice`);
    byType.set(type, m);
  }
  const sorted = [...modules].sort((a, b) => a.declaration.name.localeCompare(b.declaration.name));
  return { get: (type) => byType.get(type), all: () => sorted };
}

/**
 * Every file in `./types/` is one tile: its default export is a `defineTileModule(...)`.
 * Adding a tile is adding a file; nothing here changes.
 */
const files = import.meta.glob<{ default: TileModule }>(['./types/*.tsx', '!./types/*.test.tsx'], { eager: true });

export const registry: TileRegistry = createRegistry(Object.values(files).map((file) => file.default));
