/**
 * Temperature bands (spec section 10, Colour): the cold, in-window and hot ranges for a
 * tyre compound and for brakes. Fixed, not user-configurable.
 *
 * PROVISIONAL. These numbers are placeholders chosen to make the tile usable. The real
 * values come from recordings in the calibration build task (#52), which replaces both
 * tables below and clears `BANDS_ARE_PROVISIONAL`. Nothing else in the dashboard holds a
 * temperature threshold.
 */
export const BANDS_ARE_PROVISIONAL = true;

export interface Band {
  /** Below this, cold. */
  cold: number;
  /** Above this, hot. Between the two, inclusive, in window. */
  hot: number;
}

export type BandName = 'cold' | 'window' | 'hot';

/** Tyres by actual compound ID (`m_actualTyreCompound`), °C. Applies to surface and inner. */
export const TYRE_BANDS: Readonly<Record<number, Band>> = {
  22: { cold: 78, hot: 98 }, // C6
  16: { cold: 80, hot: 100 }, // C5
  17: { cold: 82, hot: 102 }, // C4
  18: { cold: 84, hot: 104 }, // C3
  19: { cold: 86, hot: 106 }, // C2
  20: { cold: 88, hot: 108 }, // C1
  21: { cold: 90, hot: 110 }, // C0
  7: { cold: 60, hot: 85 }, // intermediate
  8: { cold: 50, hot: 75 }, // wet
};

/** Brake discs, °C. */
export const BRAKE_BAND: Band = { cold: 300, hot: 850 };

/** Full-scale of a thermal column in °C: how tall the bar is at this temperature. Provisional too. */
export const COLUMN_SCALE_MAX = { tyre: 140, brake: 1000 } as const;

/** Cold and hot show an arrow beside the number, so colour never carries it alone. */
export const BAND_GLYPH: Readonly<Record<BandName, string>> = { cold: '▼', window: '', hot: '▲' };

/**
 * Band of a temperature in °C. A tyre with an unknown compound has no band: null, and the
 * tile shows the number without colour or glyph.
 */
export function bandOf(kind: 'tyre' | 'brake', celsius: number, compound: number | undefined): BandName | null {
  const band = kind === 'brake' ? BRAKE_BAND : compound === undefined ? undefined : TYRE_BANDS[compound];
  if (!band) return null;
  return celsius < band.cold ? 'cold' : celsius > band.hot ? 'hot' : 'window';
}
