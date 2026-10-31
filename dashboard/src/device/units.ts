/** Per-device unit choices (spec section 10, Units). Fuel stays in kg and laps: no selector. */
export interface Units {
  speed: 'kmh' | 'mph';
  temperature: 'c' | 'f';
  pressure: 'psi' | 'bar';
}

/** A new device starts here and does not follow the game's own preference. */
export const DEFAULT_UNITS: Units = { speed: 'kmh', temperature: 'c', pressure: 'psi' };

export interface Quantity {
  value: number;
  unit: string;
}

const KMH_TO_MPH = 0.621371192;
const M_TO_FT = 3.280839895;
const PSI_TO_BAR = 0.0689475729;

/** From km/h, as the wire carries it. */
export function speed(kmh: number, units: Units): Quantity {
  return units.speed === 'mph' ? { value: kmh * KMH_TO_MPH, unit: 'mph' } : { value: kmh, unit: 'km/h' };
}

/** From metres. Distance follows speed: metres with km/h, feet with mph. */
export function distance(metres: number, units: Units): Quantity {
  return units.speed === 'mph' ? { value: metres * M_TO_FT, unit: 'ft' } : { value: metres, unit: 'm' };
}

/** From °C. */
export function temperature(celsius: number, units: Units): Quantity {
  return units.temperature === 'f' ? { value: celsius * 1.8 + 32, unit: '°F' } : { value: celsius, unit: '°C' };
}

/** From psi, as the game reports tyre pressure. */
export function pressure(psi: number, units: Units): Quantity {
  return units.pressure === 'bar' ? { value: psi * PSI_TO_BAR, unit: 'bar' } : { value: psi, unit: 'psi' };
}
