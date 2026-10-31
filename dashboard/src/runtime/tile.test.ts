import { describe, expect, test } from 'vitest';
import { defineTile, frameFieldsOf, frameGroupsOf, pickDensity, type TileDeclaration } from './tile';

const base: TileDeclaration = {
  type: 'speed',
  name: 'Speed',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 2, h: 2 },
  defaultSize: { w: 3, h: 3 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 200, minHeight: 120 },
    { density: 'full', minWidth: 360, minHeight: 220 },
  ],
  reads: [
    { source: 'frame', fields: ['speed', 'gear', 'ersPercent'] },
    { source: 'state', keys: ['lifecycle'] },
  ],
};

describe('defineTile', () => {
  test('returns a valid declaration unchanged', () => {
    expect(defineTile(base)).toBe(base);
  });
  test('needs 2 or 3 densities in ascending order starting from compact', () => {
    expect(() => defineTile({ ...base, densities: [base.densities[0]!] })).toThrow(/densities/);
    expect(() => defineTile({ ...base, densities: [...base.densities].reverse() })).toThrow(/densities/);
    expect(() =>
      defineTile({
        ...base,
        densities: [
          { density: 'regular', minWidth: 0, minHeight: 0 },
          { density: 'full', minWidth: 100, minHeight: 100 },
        ],
      }),
    ).toThrow(/compact/);
  });
  test('default size is at least the minimum', () => {
    expect(() => defineTile({ ...base, defaultSize: { w: 1, h: 3 } })).toThrow(/default/);
  });
  test('a tile that reads no frame field cannot be a frame tile', () => {
    expect(() => defineTile({ ...base, reads: [{ source: 'state', keys: ['x'] }] })).toThrow(/frame/);
  });
});

describe('pickDensity: chosen from the actual size, never by scaling', () => {
  const d = (w: number, h: number) => pickDensity(base, w, h);
  test('the largest density whose thresholds both fit', () => {
    expect(d(100, 100)).toBe('compact');
    expect(d(200, 120)).toBe('regular');
    expect(d(500, 130)).toBe('regular'); // wide but short
    expect(d(360, 220)).toBe('full');
  });
  test('falls back to compact below every threshold', () => {
    expect(pickDensity({ ...base, densities: [
      { density: 'compact', minWidth: 80, minHeight: 80 },
      { density: 'regular', minWidth: 200, minHeight: 120 },
    ] }, 10, 10)).toBe('compact');
  });
});

describe('reads', () => {
  test('lists the frame fields and the readout groups behind them', () => {
    expect(frameFieldsOf(base)).toEqual(['speed', 'gear', 'ersPercent']);
    expect(frameGroupsOf(base)).toEqual(['telemetry', 'status']);
  });
});
