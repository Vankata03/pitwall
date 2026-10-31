import { carLabel, identityKey, parseIdentity, resolveIdentity, type CarIdentity, type Participant } from '../session/cars';

const FOLLOW_PLAYER = 'player';

export interface FocusPickerProps {
  participants: readonly Participant[];
  focus: CarIdentity | null;
  onChange(identity: CarIdentity | null): void;
}

/**
 * The focus car for this device. Follow player is the default; a stored focus that is not
 * in the session stays selected and says so, until the viewer chooses again.
 */
export function FocusPicker({ participants, focus, onChange }: FocusPickerProps) {
  const absent = focus !== null && resolveIdentity(focus, participants).status === 'not-in-session';
  const value = focus === null ? FOLLOW_PLAYER : identityKey(focus);
  return (
    <label className="pw-field pw-focus-picker">
      <span className="pw-label">Focus car</span>
      <select
        className="pw-select"
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
    </label>
  );
}
