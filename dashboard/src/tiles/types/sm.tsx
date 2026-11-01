import { useRef } from 'react';
import { effectiveState, frameReadoutState, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import { setData, showMessage, TileMessage, useCarState, useLifecycle, valueOf, write } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';

const declaration = {
  type: 'sm',
  name: 'SM',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 2, h: 2 },
  defaultSize: { w: 4, h: 2 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 190, minHeight: 72 },
  ],
  reads: [
    { source: 'frame', fields: ['aeroState'] },
    { source: 'state', keys: ['cars'] },
  ],
} as const;

type Kind = 'sm' | 'drs' | null;

/**
 * The slot's wording. The Frame's one `aeroState` byte is the active aero mode on a
 * 2026-regulations car and the DRS flag on a pre-2026 car. Mode values are assumed from the
 * format notes (0 off, 1 on) until a capture confirms them; anything else is named, not guessed.
 */
function stateText(kind: Exclude<Kind, null>, byte: number, available: boolean): string {
  if (byte > 1) return `MODE ${byte}`;
  if (byte === 1) return kind === 'sm' ? 'ACTIVE' : 'OPEN';
  if (available) return 'AVAILABLE';
  return kind === 'sm' ? 'INACTIVE' : 'CLOSED';
}

function Sm({ car, density }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const value = useRef<HTMLSpanElement>(null);
  const carState = useCarState(car);
  const lifecycle = useLifecycle();

  const kind: Kind =
    carState?.regulations === 'Regulations2026' ? 'sm' : carState?.regulations === 'Pre2026' ? 'drs' : null;
  const available =
    kind === 'sm'
      ? valueOf(carState?.aero)?.active_aero_available === 1
      : kind === 'drs'
        ? valueOf(carState?.status)?.drs_allowed === 1
        : false;

  useFrameTile(
    declaration,
    car,
    ({ frame, car: index, connected }) => {
      // Without the regulations the byte has no meaning: absent, whatever the frame says.
      const state =
        index === null || kind === null
          ? ReadoutState.Absent
          : effectiveState(frameReadoutState(frame, index, 'aeroState'), connected);
      const live = state === ReadoutState.Live || state === ReadoutState.Stale;
      const text = live && index !== null && kind !== null ? stateText(kind, frame.aeroState[index]!, available) : '';
      write(value.current, state, text);
      setData(value.current, 'mode', live ? text.toLowerCase().replace(/\s+/g, '-') : '');
      showMessage(root.current, wholeTileMessage([state], { lifecycle, connected }));
    },
    [kind, available, lifecycle],
  );

  const label = kind === 'sm' ? 'SM' : kind === 'drs' ? 'DRS' : 'SM / DRS';
  return (
    <div className="pw-frame-tile pw-sm" ref={root} data-density={density}>
      <div className="pw-frame-body pw-sm-body">
        <span className="pw-label" data-testid="sm-label">
          {label}
        </span>
        <span className="pw-num pw-readout pw-sm-state" ref={value} data-testid="sm-state" />
        {density !== 'compact' ? (
          <span className="pw-label" data-testid="sm-detail">
            {kind === 'sm' ? 'Straight mode' : kind === 'drs' ? 'Drag reduction' : 'Aero mode'}
          </span>
        ) : null}
      </div>
      <TileMessage />
    </div>
  );
}

export default defineTileModule({ declaration, Component: Sm });
