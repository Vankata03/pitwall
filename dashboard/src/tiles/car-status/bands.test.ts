import { expect, test } from 'vitest';
import { BAND_GLYPH, BANDS_ARE_PROVISIONAL, BRAKE_BAND, TYRE_BANDS, bandOf } from './bands';

test('the shipped bands are flagged provisional until the calibration ticket replaces them', () => {
  expect(BANDS_ARE_PROVISIONAL).toBe(true);
});

test('a tyre temperature is cold below its compound’s window, hot above it, in window between', () => {
  const { cold, hot } = TYRE_BANDS[18]!; // C3
  expect(bandOf('tyre', cold - 1, 18)).toBe('cold');
  expect(bandOf('tyre', cold, 18)).toBe('window');
  expect(bandOf('tyre', hot, 18)).toBe('window');
  expect(bandOf('tyre', hot + 1, 18)).toBe('hot');
});

test('compounds are keyed by actual compound, so a different compound gets a different window', () => {
  expect(TYRE_BANDS[16]).not.toEqual(TYRE_BANDS[21]); // C5 against C0
  const mid = (TYRE_BANDS[16]!.hot + TYRE_BANDS[16]!.cold) / 2;
  expect(bandOf('tyre', mid, 16)).toBe('window');
  expect(bandOf('tyre', 40, 7)).toBe('cold'); // a 40 °C tyre is cold on intermediates too
});

test('an unknown or missing compound has no tyre band, so the number shows uncoloured', () => {
  expect(bandOf('tyre', 95, 99)).toBeNull();
  expect(bandOf('tyre', 95, undefined)).toBeNull();
});

test('brakes use their own table and need no compound', () => {
  expect(bandOf('brake', BRAKE_BAND.cold - 1, undefined)).toBe('cold');
  expect(bandOf('brake', BRAKE_BAND.hot + 1, 16)).toBe('hot');
  expect(bandOf('brake', (BRAKE_BAND.cold + BRAKE_BAND.hot) / 2, undefined)).toBe('window');
});

test('cold and hot carry an arrow glyph, in window carries none', () => {
  expect(BAND_GLYPH.cold).toBe('▼');
  expect(BAND_GLYPH.hot).toBe('▲');
  expect(BAND_GLYPH.window).toBe('');
});
