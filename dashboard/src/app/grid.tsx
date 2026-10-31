import ReactGridLayout, { verticalCompactor, noCompactor, type Layout as GridLayout } from 'react-grid-layout';
import type { Units } from '../device/units';
import type { GridItem } from '../layout/controller';
import type { Tile } from '../layout/model';
import { COLUMNS, GRID_MARGIN, ROW_HEIGHT, cellsToPixels, streamGridMetrics, type ScreenClass } from '../layout/screen';
import type { ResolvedCar } from '../session/cars';
import { TileHost } from '../tiles/host';
import type { TileRegistry } from '../tiles/registry';
import { CloseIcon, GearIcon, MoveIcon } from './icons';

export interface DashboardGridProps {
  tiles: readonly Tile[];
  screenClass: ScreenClass;
  /** Grid width in CSS pixels. */
  width: number;
  /** Present only in Stream view: fit rows to the CSS viewport. */
  viewportHeight?: number;
  editing: boolean;
  registry: TileRegistry;
  carFor(tile: Tile): ResolvedCar | null;
  units: Units;
  onMoveResize(items: readonly GridItem[]): void;
  onOpenSettings(tile: Tile): void;
  onRemove(tile: Tile): void;
}

const GRID_CONFIG = { rowHeight: ROW_HEIGHT, margin: [GRID_MARGIN, GRID_MARGIN] as const, containerPadding: [0, 0] as const, maxRows: Infinity };
const DRAG_HANDLE = '.pw-tile-grip';
const DRAG_CANCEL = '.pw-tile-actions';

export function DashboardGrid(props: DashboardGridProps) {
  const { tiles, screenClass, width, editing, registry } = props;
  const columns = COLUMNS[screenClass];
  const streaming = props.viewportHeight !== undefined;
  const metrics = props.viewportHeight !== undefined ? streamGridMetrics(tiles, props.viewportHeight) : { rowHeight: ROW_HEIGHT, margin: GRID_MARGIN };
  const layout: GridLayout = tiles.map((t) => {
    const min = registry.get(t.type)?.declaration.minSize;
    return {
      i: t.i,
      x: t.x,
      y: t.y,
      w: Math.min(t.w, columns),
      h: t.h,
      minW: Math.min(min?.w ?? 1, columns),
      minH: min?.h ?? 1,
    };
  });

  return (
    <ReactGridLayout
      className="pw-grid"
      width={width}
      layout={layout}
      gridConfig={{ ...GRID_CONFIG, cols: columns, rowHeight: metrics.rowHeight, margin: [metrics.margin, metrics.margin] }}
      dragConfig={{ enabled: editing, bounded: false, handle: DRAG_HANDLE, cancel: DRAG_CANCEL, threshold: 3 }}
      resizeConfig={{ enabled: editing, handles: ['se'] }}
      compactor={streaming ? noCompactor : verticalCompactor}
      onLayoutChange={(next) => props.onMoveResize(next.map(({ i, x, y, w, h }) => ({ i, x, y, w, h })))}
    >
      {tiles.map((tile) => {
        const { width: px, height: py } = cellsToPixels(screenClass, width, { w: Math.min(tile.w, columns), h: tile.h }, metrics.rowHeight, metrics.margin);
        const name = registry.get(tile.type)?.declaration.name ?? tile.type;
        const configurable = registry.get(tile.type) !== undefined;
        return (
          <div key={tile.i} className="pw-cell">
            <section className="pw-tile" aria-label={name} data-tile-type={tile.type}>
              <TileHost
                tile={tile}
                registry={registry}
                width={px}
                height={py}
                car={props.carFor(tile)}
                units={props.units}
                editing={editing}
              />
              {editing ? (
                <div className="pw-tile-edit">
                  <div className="pw-tile-grip" title="Drag to move">
                    <MoveIcon />
                  </div>
                  <div className="pw-tile-actions">
                    {configurable ? (
                      <button type="button" className="pw-icon-button" aria-label={`Settings for ${name}`} onClick={() => props.onOpenSettings(tile)}>
                        <GearIcon />
                      </button>
                    ) : null}
                    <button type="button" className="pw-icon-button" aria-label={`Remove ${name}`} onClick={() => props.onRemove(tile)}>
                      <CloseIcon />
                    </button>
                  </div>
                </div>
              ) : null}
            </section>
          </div>
        );
      })}
    </ReactGridLayout>
  );
}
