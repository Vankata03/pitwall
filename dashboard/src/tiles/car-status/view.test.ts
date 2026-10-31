import { describe, expect, test } from 'vitest';
import type { CarState } from '../../generated/wire/CarState';
import type { CarDamageData } from '../../generated/wire/CarDamageData';
import type { CarTelemetryData } from '../../generated/wire/CarTelemetryData';
import type { StatusData } from '../../generated/wire/StatusData';
import type { Units } from '../../device/units';
import { carStatusView } from './view';

const metric: Units = { speed: 'kmh', temperature: 'c', pressure: 'psi' };
const four = <T,>(a: T, b: T, c: T, d: T): [T, T, T, T] => [a, b, c, d];

// Game order is rear-left, rear-right, front-left, front-right.
const telemetry = {
  brakes_temperature: four(569, 571, 587, 588),
  tyres_surface_temperature: four(97, 95, 94, 92),
  tyres_inner_temperature: four(78, 78, 80, 80),
  tyres_pressure: four(21.8, 21.7, 23.5, 23.4),
} as CarTelemetryData;
const status = { actual_tyre_compound: 18, visual_tyre_compound: 17, tyres_age_laps: 9 } as StatusData;
const damage = {
  brakes_damage: four(0, 1, 2, 3),
  front_left_wing_damage: 12,
  front_right_wing_damage: 0,
  rear_wing_damage: 4,
  floor_damage: 0,
  diffuser_damage: 0,
  sidepod_damage: 0,
  gear_box_damage: 0,
  engine_damage: 0,
} as CarDamageData;

const car = (over: Partial<CarState> = {}): CarState =>
  ({
    telemetry: { state: 'live', value: telemetry },
    status: { state: 'live', value: status },
    damage: { state: 'live', value: damage },
    tyreCondition: { state: 'live', value: four(73, 76, 79, 82) },
    ...over,
  }) as CarState;

describe('wheel positions', () => {
  test('game order RL, RR, FL, FR maps to its wheel, and each wheel reads its own values', () => {
    const v = carStatusView(car(), metric, true);
    expect(v.wheels.map((w) => w.id)).toEqual(['RL', 'RR', 'FL', 'FR']);
    const fr = v.wheels.find((w) => w.id === 'FR')!;
    expect(fr.condition.text).toBe('82%');
    expect(fr.brake.text).toBe('588');
    expect(fr.surface.text).toBe('92');
    expect(fr.inner.text).toBe('80');
    expect(fr.pressure.text).toBe('23.4');
    expect(v.wheels.find((w) => w.id === 'RL')!.condition.text).toBe('73%');
  });
});

describe('tyre condition', () => {
  test('is a number with a bar length, and colour only repeats it', () => {
    const rl = carStatusView(car(), metric, true).wheels[0]!;
    expect(rl.condition.percent).toBe(73);
    expect(rl.condition.level).toBe('healthy');
    const worn = carStatusView(car({ tyreCondition: { state: 'live', value: four(30, 55, 100, 71) } }), metric, true);
    expect(worn.wheels.map((w) => w.condition.level)).toEqual(['low', 'mid', 'healthy', 'healthy']);
    expect(worn.wheels[2]!.condition.text).toBe('100%');
  });

  test('restricted wear never shows 100 or any number', () => {
    const v = carStatusView(car({ tyreCondition: { state: 'restricted' } }), metric, true);
    for (const w of v.wheels) {
      expect(w.condition).toMatchObject({ state: 'restricted', text: 'Restricted', percent: null, level: null });
    }
  });

  test('absent wear shows an em dash, not zero', () => {
    const v = carStatusView(car({ tyreCondition: { state: 'absent' } }), metric, true);
    expect(v.wheels[0]!.condition).toMatchObject({ state: 'absent', text: '—', percent: null });
  });
});

describe('temperatures', () => {
  test('are banded by the actual compound, with an arrow glyph on cold and hot', () => {
    const t = { ...telemetry, tyres_surface_temperature: four(40, 90, 120, 90) } as CarTelemetryData;
    const v = carStatusView(car({ telemetry: { state: 'live', value: t } }), metric, true);
    expect(v.wheels.map((w) => [w.surface.band, w.surface.glyph])).toEqual([
      ['cold', '▼'],
      ['window', ''],
      ['hot', '▲'],
      ['window', ''],
    ]);
  });

  test('an unknown actual compound leaves the tyre number without band or glyph, brakes still banded', () => {
    const v = carStatusView(
      car({ status: { state: 'live', value: { ...status, actual_tyre_compound: 99 } } }),
      metric,
      true,
    );
    expect(v.wheels[0]!.surface).toMatchObject({ text: '97', band: null, glyph: '' });
    expect(v.wheels[0]!.brake.band).toBe('window');
  });

  test('a missing status leaves the tyre numbers without band', () => {
    const v = carStatusView(car({ status: { state: 'absent' } }), metric, true);
    expect(v.wheels[0]!.surface).toMatchObject({ text: '97', band: null });
    expect(v.compound).toMatchObject({ state: 'absent', text: '—' });
    expect(v.age).toMatchObject({ state: 'absent', text: '—' });
  });

  test('convert to °F and pressure to bar for the device, but band on °C', () => {
    const f: Units = { speed: 'kmh', temperature: 'f', pressure: 'bar' };
    const v = carStatusView(car(), f, true);
    expect(v.wheels[0]!.surface.text).toBe('207'); // 97 °C
    expect(v.wheels[0]!.surface.band).toBe('window');
    expect(v.wheels[0]!.pressure.text).toBe('1.50'); // 21.8 psi
    expect(v.units).toEqual({ temperature: '°F', pressure: 'bar' });
  });
});

describe('thermal column fill', () => {
  test('is the temperature as a share of the column’s scale, clamped, and null without a value', () => {
    const t = { ...telemetry, brakes_temperature: four(500, 1200, 0, 250) } as CarTelemetryData;
    const v = carStatusView(car({ telemetry: { state: 'live', value: t } }), metric, true);
    expect(v.wheels.map((w) => w.brake.fill)).toEqual([0.5, 1, 0, 0.25]);
    expect(carStatusView(car({ telemetry: { state: 'absent' } }), metric, true).wheels[0]!.brake.fill).toBeNull();
  });

  test('does not change with the display unit', () => {
    const f: Units = { speed: 'kmh', temperature: 'f', pressure: 'psi' };
    expect(carStatusView(car(), f, true).wheels[0]!.brake.fill).toBe(carStatusView(car(), metric, true).wheels[0]!.brake.fill);
  });
});

describe('missing data keeps each readout in its own state', () => {
  test('restricted damage and wear do not touch the temperatures', () => {
    const v = carStatusView(car({ damage: { state: 'restricted' }, tyreCondition: { state: 'restricted' } }), metric, true);
    expect(v.wheels[0]!.surface.state).toBe('live');
    expect(v.damage.every((d) => d.cell.state === 'restricted')).toBe(true);
    expect(v.wheels[0]!.brakeDamage.state).toBe('restricted');
  });

  test('absent telemetry shows em dashes, never zero', () => {
    const v = carStatusView(car({ telemetry: { state: 'absent' } }), metric, true);
    for (const w of v.wheels) {
      for (const c of [w.brake, w.surface, w.inner]) expect(c).toMatchObject({ state: 'absent', text: '—', band: null });
      expect(w.pressure).toMatchObject({ state: 'absent', text: '—' });
    }
  });

  test('while the collector is unreachable, live values read stale and keep their value', () => {
    const v = carStatusView(car(), metric, false);
    expect(v.wheels[0]!.surface).toMatchObject({ state: 'stale', text: '97' });
    expect(v.wheels[0]!.condition).toMatchObject({ state: 'stale', text: '73%' });
  });

  test('a car with no data at all reports it has none, for the whole-tile message', () => {
    const empty = { telemetry: { state: 'absent' }, status: { state: 'absent' }, damage: { state: 'absent' }, tyreCondition: { state: 'absent' } } as CarState;
    expect(carStatusView(empty, metric, true).hasData).toBe(false);
    expect(carStatusView(undefined, metric, true).hasData).toBe(false);
    expect(carStatusView(car(), metric, true).hasData).toBe(true);
  });
});

describe('compound, age and damage', () => {
  test('shows the visual compound name and the age in laps', () => {
    const v = carStatusView(car(), metric, true);
    expect(v.compound).toMatchObject({ state: 'live', text: 'Medium' });
    expect(v.age).toMatchObject({ state: 'live', text: '9' });
  });

  test('damage is numeric percent per part', () => {
    const v = carStatusView(car(), metric, true);
    const get = (key: string) => v.damage.find((d) => d.key === key)!.cell.text;
    expect(get('front-left-wing')).toBe('12%');
    expect(get('front-right-wing')).toBe('0%');
    expect(get('rear-wing')).toBe('4%');
    expect(v.wheels.map((w) => w.brakeDamage.text)).toEqual(['0%', '1%', '2%', '3%']);
  });
});
