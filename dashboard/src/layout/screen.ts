import type { Density, DensityRule, TileSize } from '../runtime/tile';

export type ScreenClass = 'wide' | 'tablet' | 'phone';

/** Columns per screen class (spec section 10, Grid). */
export const COLUMNS: Record<ScreenClass, number> = { wide: 24, tablet: 12, phone: 4 };

/** Narrowest viewport of each class, in CSS pixels. */
export const CLASS_MIN_WIDTH: Record<ScreenClass, number> = { wide: 1280, tablet: 768, phone: 0 };

export const ROW_HEIGHT = 32;
const MAX_FIT_ROW_HEIGHT = 64;
export const GRID_MARGIN = 8;
/** Space between the viewport edge and the grid, in CSS pixels. */
export const PAGE_PADDING = 8;
/** A classic vertical scrollbar; the page scrolls when a layout is taller than the screen. */
const SCROLLBAR = 15;

export function screenClassOf(viewportWidth: number): ScreenClass {
  if (viewportWidth >= CLASS_MIN_WIDTH.wide) return 'wide';
  if (viewportWidth >= CLASS_MIN_WIDTH.tablet) return 'tablet';
  return 'phone';
}

/** CSS pixels a block of grid cells occupies in a grid `containerWidth` wide. */
export function cellsToPixels(
  screenClass: ScreenClass,
  containerWidth: number,
  size: TileSize,
  rowHeight = ROW_HEIGHT,
  margin = GRID_MARGIN,
): { width: number; height: number } {
  const columns = COLUMNS[screenClass];
  const column = (containerWidth - margin * (columns - 1)) / columns;
  return {
    width: size.w * column + Math.max(0, size.w - 1) * margin,
    height: size.h * rowHeight + Math.max(0, size.h - 1) * margin,
  };
}

/**
 * Container width of the narrowest wide screen: the 1280 px floor of the class, less a
 * scrollbar and the page padding. A cell is narrowest here (24 columns on the least width),
 * so a threshold that holds at this width holds on every wider screen and on tablet and
 * phone, which give each cell more room.
 */
export const NARROWEST_WIDE_CONTAINER = CLASS_MIN_WIDTH.wide - SCROLLBAR - 2 * PAGE_PADDING;

export interface DensityCells {
  density: Density;
  /** Smallest tile, in grid cells, that gets this density. */
  w: number;
  h: number;
}

/**
 * Density rules written in grid cells, converted to pixels for the narrowest wide screen.
 * Declare thresholds this way so a tile that a preset sizes at, say, 9×16 reliably reaches
 * its Full density at 1280 px, instead of sitting a few pixels short.
 */
export function densityRulesFromCells(cells: readonly DensityCells[]): DensityRule[] {
  return cells.map(({ density, w, h }) => {
    if (w === 0 && h === 0) return { density, minWidth: 0, minHeight: 0 };
    const px = cellsToPixels('wide', NARROWEST_WIDE_CONTAINER, { w, h });
    return { density, minWidth: Math.floor(px.width), minHeight: Math.floor(px.height) };
  });
}

/**
 * Row height that lets a layout fill the height left under the chrome. ROW_HEIGHT is the floor,
 * so a layout taller than the screen still scrolls, and MAX_FIT_ROW_HEIGHT keeps a short layout
 * from ballooning. Whole pixels, so tile edges stay crisp.
 */
export function fitRowHeight(tiles: readonly { y: number; h: number }[], available: number): number {
  const rows = Math.max(1, ...tiles.map((tile) => tile.y + tile.h));
  const fit = Math.floor((available - GRID_MARGIN * (rows - 1)) / rows);
  return Math.min(MAX_FIT_ROW_HEIGHT, Math.max(ROW_HEIGHT, fit));
}

/** Divide the viewport between rows after reserving gaps, so the last row never scrolls.
 * Count positions as well as heights: intentional empty rows remain empty on air. */
export function streamGridMetrics(tiles: readonly { y: number; h: number }[], viewportHeight: number): { rowHeight: number; margin: number } {
  const rows = Math.max(1, ...tiles.map((tile) => tile.y + tile.h));
  const height = Math.max(0, viewportHeight);
  const margin = Math.min(GRID_MARGIN, height / (2 * rows));
  return { rowHeight: (height - margin * (rows - 1)) / rows, margin };
}
