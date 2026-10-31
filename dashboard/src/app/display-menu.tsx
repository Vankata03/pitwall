import type { ReactNode } from 'react';
import type { Units } from '../device/units';
import { Popover } from './popover';

const CHOICES: { key: keyof Units; legend: string; options: { value: string; label: string }[] }[] = [
  { key: 'speed', legend: 'Speed', options: [{ value: 'kmh', label: 'km/h' }, { value: 'mph', label: 'mph' }] },
  { key: 'temperature', legend: 'Temperature', options: [{ value: 'c', label: '°C' }, { value: 'f', label: '°F' }] },
  { key: 'pressure', legend: 'Pressure', options: [{ value: 'psi', label: 'psi' }, { value: 'bar', label: 'bar' }] },
];

export interface DisplayMenuProps {
  units: Units;
  onChange(units: Units): void;
  /** Extra notices for this device, such as the slow-draw notice. */
  children?: ReactNode;
}

/** Per-device units. Distance follows speed and fuel stays in kg and laps, so there are three choices. */
export function DisplayMenu({ units, onChange, children }: DisplayMenuProps) {
  return (
    <Popover label="Display" panelLabel="Display menu" className="pw-display-menu">
      {() => (
        <div className="pw-display-body">
          {CHOICES.map(({ key, legend, options }) => (
            <fieldset key={key} className="pw-fieldset">
              <legend className="pw-label">{legend}</legend>
              <div className="pw-segmented">
                {options.map((o) => (
                  <label key={o.value} className="pw-segment">
                    <input
                      type="radio"
                      name={`pw-units-${key}`}
                      value={o.value}
                      checked={units[key] === o.value}
                      onChange={() => onChange({ ...units, [key]: o.value })}
                    />
                    <span>{o.label}</span>
                  </label>
                ))}
              </div>
            </fieldset>
          ))}
          {children}
        </div>
      )}
    </Popover>
  );
}
