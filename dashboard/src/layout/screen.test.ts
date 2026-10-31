import { describe, expect, test } from 'vitest';
import { defineTile, pickDensity } from '../runtime/tile';
import {
  COLUMNS,
  GRID_MARGIN,
  ROW_HEIGHT,
  cellsToPixels,
  densityRulesFromCells,
  screenClassOf,
} from './screen';

describe('screen classes (spec section 10, Grid)', () => {
  test.each([
    [1920, 'wide'],
    [1280, 'wide'],
    [1279, 'tablet'],
    [768, 'tablet'],
    [767, 'phone'],
    [320, 'phone'],
  ] as const)('a %ipx viewport is %s', (width, expected) => {
    expect(screenClassOf(width)).toBe(expected);
  });

  test('columns per class', () => {
    expect(COLUMNS).toEqual({ wide: 24, tablet: 12, phone: 4 });
  });

  test('row height is 32 px with an 8 px margin', () => {
    expect(ROW_HEIGHT).toBe(32);
    expect(GRID_MARGIN).toBe(8);
  });
});

describe('cell sizes in pixels', () => {
  test('height is rows plus the gaps between them', () => {
    expect(cellsToPixels('wide', 1280, { w: 1, h: 1 }).height).toBe(32);
    expect(cellsToPixels('wide', 1280, { w: 1, h: 16 }).height).toBe(16 * 32 + 15 * 8);
  });

  test('width is columns plus gaps, from the container width', () => {
    // 1264 px container, 24 columns, 23 gaps of 8: column = (1264 - 184) / 24 = 45 px.
    expect(cellsToPixels('wide', 1264, { w: 1, h: 1 }).width).toBe(45);
    expect(cellsToPixels('wide', 1264, { w: 9, h: 1 }).width).toBe(9 * 45 + 8 * 8);
  });
});

describe('density thresholds written in grid cells', () => {
  const rules = densityRulesFromCells([
    { density: 'compact', w: 0, h: 0 },
    { density: 'regular', w: 5, h: 8 },
    { density: 'full', w: 9, h: 16 },
  ]);

  test('Race preset timing tower, 9x16 at the narrowest wide screen, reaches Full', () => {
    // 1280 px viewport less a 15 px scrollbar and 8 px page padding each side.
    const container = 1280 - 15 - 16;
    const px = cellsToPixels('wide', container, { w: 9, h: 16 });
    const tile = defineTile({
      type: 'density-probe',
      name: 'Probe',
      binding: 'session-wide',
      rate: 'slow',
      minSize: { w: 4, h: 6 },
      defaultSize: { w: 6, h: 12 },
      densities: rules,
      reads: [],
    });
    expect(pickDensity(tile, px.width, px.height)).toBe('full');
  });

  test('one cell narrower or shorter than the threshold stays Regular', () => {
    const container = 1280 - 15 - 16;
    const tile = defineTile({
      type: 'density-probe',
      name: 'Probe',
      binding: 'session-wide',
      rate: 'slow',
      minSize: { w: 4, h: 6 },
      defaultSize: { w: 6, h: 12 },
      densities: rules,
      reads: [],
    });
    const narrow = cellsToPixels('wide', container, { w: 8, h: 16 });
    const short = cellsToPixels('wide', container, { w: 9, h: 15 });
    expect(pickDensity(tile, narrow.width, narrow.height)).toBe('regular');
    expect(pickDensity(tile, short.width, short.height)).toBe('regular');
  });

  test('wider screens only help', () => {
    const tile = defineTile({
      type: 'density-probe',
      name: 'Probe',
      binding: 'session-wide',
      rate: 'slow',
      minSize: { w: 4, h: 6 },
      defaultSize: { w: 6, h: 12 },
      densities: rules,
      reads: [],
    });
    const px = cellsToPixels('wide', 1904, { w: 9, h: 16 });
    expect(pickDensity(tile, px.width, px.height)).toBe('full');
  });
});
