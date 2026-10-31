import { describe, expect, test } from 'vitest';
import {
  DEFAULT_UNITS,
  distance,
  pressure,
  speed,
  temperature,
  type Units,
} from './units';

const mph: Units = { ...DEFAULT_UNITS, speed: 'mph' };

describe('units (spec section 10, Units)', () => {
  test('defaults do not follow the game: km/h, °C, psi', () => {
    expect(DEFAULT_UNITS).toEqual({ speed: 'kmh', temperature: 'c', pressure: 'psi' });
  });

  test('speed arrives in km/h', () => {
    expect(speed(100, DEFAULT_UNITS)).toEqual({ value: 100, unit: 'km/h' });
    const converted = speed(100, mph);
    expect(converted.unit).toBe('mph');
    expect(converted.value).toBeCloseTo(62.137, 3);
  });

  test('distance follows speed: metres with km/h, feet with mph', () => {
    expect(distance(100, DEFAULT_UNITS)).toEqual({ value: 100, unit: 'm' });
    const feet = distance(100, mph);
    expect(feet.unit).toBe('ft');
    expect(feet.value).toBeCloseTo(328.084, 3);
  });

  test('temperature arrives in °C', () => {
    expect(temperature(100, DEFAULT_UNITS)).toEqual({ value: 100, unit: '°C' });
    expect(temperature(100, { ...DEFAULT_UNITS, temperature: 'f' })).toEqual({ value: 212, unit: '°F' });
    expect(temperature(-40, { ...DEFAULT_UNITS, temperature: 'f' }).value).toBe(-40);
  });

  test('pressure arrives in psi', () => {
    expect(pressure(22, DEFAULT_UNITS)).toEqual({ value: 22, unit: 'psi' });
    const bar = pressure(22, { ...DEFAULT_UNITS, pressure: 'bar' });
    expect(bar.unit).toBe('bar');
    expect(bar.value).toBeCloseTo(1.5168, 4);
  });
});
