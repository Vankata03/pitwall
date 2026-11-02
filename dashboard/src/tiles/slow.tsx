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
export interface Row {
  label: string;
  value: Readout<string>;
  /** A quiet line under the label, such as which car a gap is to. */
  note?: string | undefined;
  /** Accent for the value; colour only repeats what the value's text or sign already says. */
  tone?: 'good' | 'bad' | 'accent' | undefined;
  /** A decorative glyph beside the value; the value's text still carries the meaning. */
  icon?: ReactNode;
}
export function SlowTile({ title, snapshot, rows, children }: { title: string; snapshot: SlowSnapshot; rows: Row[]; children?: ReactNode }) {
  const connected = snapshot.connection === 'open';
  const message = wholeTileMessage(rows.map(r => effectiveReadoutState(r.value, connected)), { connected, lifecycle: lifecycleOf(snapshot.state) });
  return <section className="pw-slow-tile" aria-label={title}>
    <div className="pw-slow-readouts">{rows.map(r => <div className="pw-slow-row" key={r.label} data-tone={r.tone}>
      <span className="pw-label">{r.label}</span>
      {r.icon ? <span className="pw-slow-icon" aria-hidden="true">{r.icon}</span> : null}
      <Value state={effectiveReadoutState(r.value, connected)} value={'value' in r.value ? r.value.value : ''} />
      {r.note ? <span className="pw-label pw-slow-note">{r.note}</span> : null}
    </div>)}</div>
    {children}
    {message ? <span className="pw-slow-message pw-label" role="status">{message}</span> : null}
  </section>;
}
