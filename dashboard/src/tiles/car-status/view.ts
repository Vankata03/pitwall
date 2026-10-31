/**
 * The Car status tile as data: everything the tile prints, decided here so the component
 * only lays it out. Reads the generated `CarState` (State, slow path) and applies the
 * shared readout rule (spec section 10, Missing data), units and temperature bands.
 *
 * Wheel arrays on the wire are in the game's order: rear-left, rear-right, front-left,
 * front-right.
 */
import { pressure as toPressure, temperature as toTemperature, type Units } from '../../device/units';
import type { CarState } from '../../generated/wire/CarState';
import type { CarTelemetryData } from '../../generated/wire/CarTelemetryData';
import { jsonReadoutView, type ReadoutStateName } from '../../runtime/readout';
import type { Readout } from '../../generated/wire/Readout';
import { BAND_GLYPH, COLUMN_SCALE_MAX, bandOf, type BandName } from './bands';

export type WheelId = 'RL' | 'RR' | 'FL' | 'FR';
const WHEELS: readonly WheelId[] = ['RL', 'RR', 'FL', 'FR'];

export interface Cell {
  state: ReadoutStateName;
  text: string;
}

/** A temperature cell: `band` is null for an unknown compound or a missing value. */
export interface TemperatureCell extends Cell {
  band: BandName | null;
  /** Arrow beside the number for cold and hot; empty otherwise. */
  glyph: string;
  /** Column height, 0 to 1 of the column's scale, in °C whatever the display unit; null without a value. */
  fill: number | null;
}

export type ConditionLevel = 'healthy' | 'mid' | 'low';

export interface ConditionCell extends Cell {
  /** 0 to 100, or null unless live or stale: it sets the bar length. */
  percent: number | null;
  level: ConditionLevel | null;
}

export interface WheelView {
  id: WheelId;
  condition: ConditionCell;
  brake: TemperatureCell;
  surface: TemperatureCell;
  inner: TemperatureCell;
  pressure: Cell;
  brakeDamage: Cell;
}

export interface DamageView {
  key: string;
  label: string;
  cell: Cell;
}

export interface CarStatusView {
  wheels: WheelView[];
  /** The visual compound's name. */
  compound: Cell;
  /** Laps on the fitted set. */
  age: Cell;
  damage: DamageView[];
  /** Units of the converted temperature and pressure numbers. */
  units: { temperature: string; pressure: string };
  /** False when no readout has data: the tile then shows a whole-tile message. */
  hasData: boolean;
}

/** Condition thresholds, in percent remaining. Not in the spec; colour only repeats the number. */
const MID_BELOW = 70;
const LOW_BELOW = 40;

const levelOf = (percent: number): ConditionLevel =>
  percent < LOW_BELOW ? 'low' : percent < MID_BELOW ? 'mid' : 'healthy';

/** Visual compound IDs (`m_visualTyreCompound`). */
const VISUAL_COMPOUNDS: Readonly<Record<number, string>> = {
  16: 'Soft',
  17: 'Medium',
  18: 'Hard',
  7: 'Inter',
  8: 'Wet',
};

const ABSENT: Readout<never> = { state: 'absent' };

/** While the collector is unreachable a live value is only the last one seen. */
function seen<T>(readout: Readout<T> | undefined, connected: boolean): Readout<T> {
  if (!readout) return ABSENT;
  return !connected && readout.state === 'live' ? { state: 'stale', value: readout.value } : readout;
}

const hasValue = <T>(r: Readout<T>): r is Extract<Readout<T>, { value: T }> => 'value' in r;

const cell = <T>(readout: Readout<T>, format: (value: T) => string): Cell => {
  const { state, text } = jsonReadoutView(readout, format);
  return { state, text };
};

const percentText = (n: number) => `${Math.round(n)}%`;
const clampPercent = (n: number) => Math.min(100, Math.max(0, n));

const DAMAGE_PARTS = [
  ['front-left-wing', 'Front left wing', 'front_left_wing_damage'],
  ['front-right-wing', 'Front right wing', 'front_right_wing_damage'],
  ['rear-wing', 'Rear wing', 'rear_wing_damage'],
  ['floor', 'Floor', 'floor_damage'],
  ['diffuser', 'Diffuser', 'diffuser_damage'],
  ['sidepod', 'Sidepod', 'sidepod_damage'],
  ['gearbox', 'Gearbox', 'gear_box_damage'],
  ['engine', 'Engine', 'engine_damage'],
] as const;

export function carStatusView(car: CarState | null | undefined, units: Units, connected: boolean): CarStatusView {
  const telemetry = seen(car?.telemetry, connected);
  const status = seen(car?.status, connected);
  const damage = seen(car?.damage, connected);
  const condition = seen(car?.tyreCondition, connected);

  const actualCompound = hasValue(status) ? status.value.actual_tyre_compound : undefined;
  const unit = { temperature: toTemperature(0, units).unit, pressure: toPressure(0, units).unit };

  const temperature = (kind: 'tyre' | 'brake', pick: (t: CarTelemetryData) => number): TemperatureCell => {
    const base = cell(telemetry, (t) => Math.round(toTemperature(pick(t), units).value).toString());
    if (!hasValue(telemetry)) return { ...base, band: null, glyph: '', fill: null };
    const celsius = pick(telemetry.value);
    const band = bandOf(kind, celsius, actualCompound);
    const fill = Math.min(1, Math.max(0, celsius / COLUMN_SCALE_MAX[kind]));
    return { ...base, band, glyph: band ? BAND_GLYPH[band] : '', fill };
  };

  const wheels = WHEELS.map((id, i): WheelView => {
    const percent = hasValue(condition) ? clampPercent(condition.value[i]!) : null;
    return {
      id,
      condition: {
        ...cell(condition, (v) => percentText(clampPercent(v[i]!))),
        percent,
        level: percent === null ? null : levelOf(percent),
      },
      brake: temperature('brake', (t) => t.brakes_temperature[i]!),
      surface: temperature('tyre', (t) => t.tyres_surface_temperature[i]!),
      inner: temperature('tyre', (t) => t.tyres_inner_temperature[i]!),
      pressure: cell(telemetry, (t) => {
        const p = toPressure(t.tyres_pressure[i]!, units);
        return p.value.toFixed(units.pressure === 'bar' ? 2 : 1);
      }),
      brakeDamage: cell(damage, (d) => percentText(d.brakes_damage[i]!)),
    };
  });

  const compound = cell(status, (s) => VISUAL_COMPOUNDS[s.visual_tyre_compound] ?? 'Unknown');
  const age = cell(status, (s) => String(s.tyres_age_laps));
  const damageViews = DAMAGE_PARTS.map(([key, label, field]) => ({
    key,
    label,
    cell: cell(damage, (d) => percentText(d[field])),
  }));

  const cells: Cell[] = [
    compound,
    age,
    ...damageViews.map((d) => d.cell),
    ...wheels.flatMap((w) => [w.condition, w.brake, w.surface, w.inner, w.pressure, w.brakeDamage]),
  ];
  return {
    wheels,
    compound,
    age,
    damage: damageViews,
    units: unit,
    hasData: cells.some((c) => c.state === 'live' || c.state === 'stale'),
  };
}
