import { carLabel, carSettingOf, identityKey, parseIdentity, resolveIdentity, type CarBindingSetting, type CarTarget, type Participant } from '../session/cars';
import type { Tile } from '../layout/model';
import type { TileModule } from '../tiles/contract';
import { Dialog } from './dialog';

const TARGETS: { value: CarTarget; label: string }[] = [
  { value: 'focus', label: 'Focus car' },
  { value: 'pinned', label: 'Pinned driver' },
  { value: 'car-ahead', label: 'Car ahead' },
  { value: 'car-behind', label: 'Car behind' },
];

export interface TileSettingsDialogProps {
  tile: Tile;
  module: TileModule;
  participants: readonly Participant[];
  onChange(settings: Record<string, unknown>): void;
  onClose(): void;
}

/** Settings of one tile instance: the car it follows, then whatever the tile itself declares. */
export function TileSettingsDialog({ tile, module, participants, onChange, onClose }: TileSettingsDialogProps) {
  const { declaration, Settings } = module;
  const follows = declaration.binding === 'follows-car';
  const car = carSettingOf(tile.settings);

  const setCar = (next: CarBindingSetting) => onChange({ ...tile.settings, car: next });
  const pinMissing = car.pin !== undefined && resolveIdentity(car.pin, participants).status === 'not-in-session';

  return (
    <Dialog title={`${declaration.name} settings`} onClose={onClose}>
      {follows ? (
        <div className="pw-settings-group">
          <label className="pw-field">
            <span className="pw-label">Follows</span>
            <select
              className="pw-select"
              value={car.target}
              onChange={(e) => setCar({ ...car, target: e.target.value as CarTarget })}
            >
              {TARGETS.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </select>
          </label>
          {car.target === 'pinned' ? (
            <label className="pw-field">
              <span className="pw-label">Driver</span>
              <select
                className="pw-select"
                value={car.pin ? identityKey(car.pin) : ''}
                onChange={(e) => {
                  const pin = parseIdentity(e.target.value);
                  setCar(pin ? { target: 'pinned', pin } : { target: 'pinned' });
                }}
              >
                {car.pin === undefined ? <option value="">Choose a driver</option> : null}
                {pinMissing && car.pin ? <option value={identityKey(car.pin)}>Car not in session</option> : null}
                {participants.map((p) => (
                  <option key={p.index} value={identityKey(p.identity)}>
                    {carLabel(p)}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </div>
      ) : null}
      {Settings ? <Settings settings={tile.settings} onChange={onChange} /> : null}
      <div className="pw-dialog-actions">
        <button type="button" className="pw-button" onClick={onClose}>
          Close
        </button>
      </div>
    </Dialog>
  );
}
