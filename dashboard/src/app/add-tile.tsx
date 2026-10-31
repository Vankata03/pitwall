import type { TileSize } from '../runtime/tile';
import type { TileRegistry } from '../tiles/registry';
import { PlusIcon } from './icons';
import { Popover } from './popover';

/** Edit mode: pick a tile type to add under the others at its default size. */
export function AddTile({ registry, onAdd }: { registry: TileRegistry; onAdd(type: string, size: TileSize): void }) {
  return (
    <Popover label="Add tile" panelLabel="Add a tile" className="pw-add-tile">
      {(close) => (
        <ul className="pw-menu-list">
          {registry.all().map(({ declaration: d }) => (
            <li key={d.type} className="pw-menu-row">
              <button
                type="button"
                className="pw-button pw-menu-choice"
                onClick={() => {
                  onAdd(d.type, d.defaultSize);
                  close();
                }}
              >
                <PlusIcon />
                <span>{d.name}</span>
                <span className="pw-label">
                  {d.defaultSize.w}×{d.defaultSize.h}
                  {d.binding === 'follows-car' ? ' · follows car' : ''}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </Popover>
  );
}
