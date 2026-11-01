import type { ReactNode } from 'react';
import type { Readout } from '../generated/wire/Readout';
import { lifecycleOf } from '../runtime/wire';
import type { SlowSnapshot } from '../runtime/stores';
import { Readout as Value } from '../runtime/react';
import { effectiveReadoutState, wholeTileMessage } from '../runtime/readout';
import type { TileProps } from './contract';
import './slow.css';

export function boundCar(snapshot: SlowSnapshot, car: TileProps['car']) {
  return car && 'index' in car ? snapshot.state?.cars?.[car.index] : null;
}
export function mapValue<T, U>(readout: Readout<T> | undefined, fn: (value: T) => U | null | undefined): Readout<U> {
  if (!readout) return { state: 'absent' };
  if (!('value' in readout)) return readout;
  const value = fn(readout.value);
  return value === null || value === undefined ? { state: 'absent' } : { state: readout.state, value };
}
export interface Row { label: string; value: Readout<string>; }
export function SlowTile({ title, snapshot, rows, children }: { title: string; snapshot: SlowSnapshot; rows: Row[]; children?: ReactNode }) {
  const connected = snapshot.connection === 'open';
  const message = wholeTileMessage(rows.map(r => effectiveReadoutState(r.value, connected)), { connected, lifecycle: lifecycleOf(snapshot.state) });
  return <section className="pw-slow-tile" aria-label={title}>
    <div className="pw-slow-readouts">{rows.map(r => <div className="pw-slow-row" key={r.label}>
      <span className="pw-label">{r.label}</span>
      <Value state={effectiveReadoutState(r.value, connected)} value={'value' in r.value ? r.value.value : ''} />
    </div>)}</div>
    {children}
    {message ? <span className="pw-slow-message pw-label" role="status">{message}</span> : null}
  </section>;
}
