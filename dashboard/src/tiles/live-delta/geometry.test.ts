import { expect, test } from 'vitest';
import { deltaGeometry } from './geometry';

test('the headline stays huge at every size, and fits its box', () => {
  for (const [w, h] of [[300, 112], [220, 112], [320, 232], [700, 232], [1450, 232], [1450, 400], [400, 600]] as const) {
    for (const density of ['compact', 'regular', 'full'] as const) {
      const g = deltaGeometry(w, h, density);
      expect(g.fontSize, `${w}x${h} ${density}`).toBeGreaterThanOrEqual(36);
      expect(g.fontSize * 5.2, 'eight characters fit the width').toBeLessThanOrEqual(w - 32 + 0.5);
      expect(g.fontSize + 16, 'and the height').toBeLessThanOrEqual(h - 24 + 0.5);
    }
  }
});

test('compact is the headline alone; larger tiles add a graph that fits', () => {
  expect(deltaGeometry(300, 112, 'compact').chart).toBeNull();
  const wide = deltaGeometry(1450, 232, 'full');
  expect(wide.arrangement).toBe('side');
  expect(wide.chart!.width).toBeGreaterThan(700);
  expect(wide.chart!.height).toBeGreaterThan(150);
  const stacked = deltaGeometry(700, 232, 'regular');
  expect(stacked.arrangement).toBe('stacked');
  expect(stacked.chart!.width).toBe(700 - 32);
  expect(stacked.chart!.height).toBeGreaterThanOrEqual(60);
  const tall = deltaGeometry(400, 600, 'full');
  expect(tall.chart!.height).toBeGreaterThan(250);
});

test('a graph too small to read is left out instead of squashed', () => {
  expect(deltaGeometry(300, 150, 'regular').chart).toBeNull();
});

test('geometry is whole pixels', () => {
  const g = deltaGeometry(1001, 233, 'full');
  expect(Number.isInteger(g.fontSize)).toBe(true);
  expect(Number.isInteger(g.chart!.width) && Number.isInteger(g.chart!.height)).toBe(true);
});
