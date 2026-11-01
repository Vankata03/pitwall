import { carLabel, identityKey, parseIdentity, resolveIdentity, type CarIdentity, type Participant } from '../session/cars';

import { ChevronIcon } from './icons';

const FOLLOW_PLAYER = 'player';

export interface FocusPickerProps {
  participants: readonly Participant[];
  focus: CarIdentity | null;
  /** The car the device is focused on, for the header. */
  focused: Participant | undefined;
  onChange(identity: CarIdentity | null): void;
}

/** How the header names the car: the driver and race number, or the number alone when the name is withheld. */
function nameOf(car: Participant | undefined): string {
  if (!car) return '—';
  return car.showOnlineNames ? `${carLabel(car)} · ${String(car.raceNumber).padStart(2, '0')}` : carLabel(car);
}

/**
 * The focus car for this device. Follow player is the default; a stored focus that is not
 * in the session stays selected and says so, until the viewer chooses again. The header shows
 * the car by name; the native select sits invisibly over it, so the name itself is the control.
 */
export function FocusPicker({ participants, focus, focused, onChange }: FocusPickerProps) {
  const absent = focus !== null && resolveIdentity(focus, participants).status === 'not-in-session';
  const value = focus === null ? FOLLOW_PLAYER : identityKey(focus);
  return (
    <div className="pw-focus-picker">
      <span className="pw-label" aria-hidden="true">
        Current driver / focus car
      </span>
      <div className="pw-focus-name">
        <span data-testid="focus-name">{absent ? 'Car not in session' : nameOf(focused)}</span>
        <ChevronIcon />
        <select
          aria-label="Focus car"
          value={value}
          onChange={(e) => onChange(e.target.value === FOLLOW_PLAYER ? null : parseIdentity(e.target.value))}
        >
          <option value={FOLLOW_PLAYER}>Follow player</option>
          {absent ? <option value={identityKey(focus)}>Car not in session</option> : null}
          {participants.map((p) => (
            <option key={p.index} value={identityKey(p.identity)}>
              {carLabel(p)}
              {p.retired ? ' (retired)' : ''}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
