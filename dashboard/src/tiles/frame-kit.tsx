/**
 * Shared parts of the frame-rate tiles: reading the followed car's State on the slow path,
 * the whole-tile message, slow readouts with the four states, and Canvas 2D sizing.
 * Frame values themselves are written straight into the DOM from `draw()`; nothing here
 * puts 60 Hz data into React state.
 */
import type { ReactNode } from 'react';
import type { CarState } from '../generated/wire/CarState';
import { effectiveReadoutState, readoutView, writeReadout, type ReadoutElement } from '../runtime/readout';
import { useSlow } from '../runtime/react';
import { lifecycleOf, ReadoutState, type Lifecycle } from '../runtime/wire';
import type { ResolvedCar } from '../session/cars';

/** The followed car's State, or null when there is none. Changes at State rate (4 to 10 Hz). */
export function useCarState(car: ResolvedCar | null): CarState | null {
  const index = car && (car.status === 'ok' || car.status === 'retired') ? car.index : null;
  return useSlow((s) => (index === null ? null : (s.state?.cars[index] ?? null)));
}

export function useLifecycle(): Lifecycle | undefined {
  return useSlow((s) => lifecycleOf(s.state));
}

export function useConnected(): boolean {
  return useSlow((s) => s.connection === 'open');
}

/** Session type from State; the game's id 18 is Time Trial. */
export function useSessionType(): number | null {
  return useSlow((s) => (s.state?.session?.state === 'live' || s.state?.session?.state === 'stale' ? s.state.session.value.sessionType : null));
}
export const TIME_TRIAL = 18;

// ---- Slow readouts ----------------------------------------------------------

type JsonReadout<T> = { state: 'live'; value: T } | { state: 'stale'; value: T } | { state: 'restricted' } | { state: 'absent' };
/** JSON and frame tiles share the same readout conversion. */
export const slowState = effectiveReadoutState;

export function valueOf<T>(readout: JsonReadout<T> | null | undefined): T | undefined {
  return readout && 'value' in readout ? readout.value : undefined;
}

/** A readout whose value comes from State: the same four states as a drawn one. */
export function SlowReadout<T>({
  readout,
  connected,
  format,
  testId,
}: {
  readout: JsonReadout<T> | null | undefined;
  connected: boolean;
  /** Called only with a value; missing and withheld data never reach it. */
  format: (value: T) => string;
  testId?: string;
}) {
  const value = valueOf(readout);
  const view = readoutView(slowState(readout, connected), value === undefined ? '' : format(value));
  return (
    <span className="pw-readout" data-state={view.state} data-testid={testId}>
      {view.text}
    </span>
  );
}

/** A readout built from text the tile already decided, with the state it should show under. */
export function StateReadout({ state, text, testId }: { state: ReadoutState; text: string; testId?: string }) {
  const view = readoutView(state, text);
  return (
    <span className="pw-readout" data-state={view.state} data-testid={testId}>
      {view.text}
    </span>
  );
}

// ---- Title ------------------------------------------------------------------

/** A tile's title row, with an optional quiet note on the right. */
export function TileTitle({ children, note }: { children: ReactNode; note?: ReactNode }) {
  return (
    <div className="pw-title">
      <span>{children}</span>
      {note ? <span className="pw-title-note">{note}</span> : null}
    </div>
  );
}

// ---- Whole-tile message -----------------------------------------------------

/**
 * The message slot every frame tile carries. It stays in the document so the tile keeps
 * its shape; `showMessage` fills it and hides the values behind it.
 */
export function TileMessage() {
  return <div className="pw-frame-message" data-testid="tile-message" role="status" hidden />;
}

export function showMessage(root: HTMLElement | null, message: string | null): void {
  if (!root) return;
  const slot = root.querySelector<HTMLElement>('.pw-frame-message');
  const empty = message !== null;
  if (String(empty) !== root.dataset.empty) root.dataset.empty = String(empty);
  if (!slot) return;
  const text = message ?? '';
  if (slot.textContent !== text) slot.textContent = text;
  if (slot.hidden === empty) slot.hidden = !empty;
}

// ---- Text and canvas --------------------------------------------------------

/** `writeReadout` for a node a density may not render. */
export function write(el: ReadoutElement | null, state: ReadoutState, text: string): void {
  if (el) writeReadout(el, state, text);
}

export function setText(el: { textContent: string | null } | null, text: string): void {
  if (el && el.textContent !== text) el.textContent = text;
}

export function setData(el: HTMLElement | null, key: string, value: string): void {
  if (el && el.dataset[key] !== value) el.dataset[key] = value;
}

/** 11,240 */
export const grouped = (n: number): string => Math.round(n).toLocaleString('en-US');

/** Real minus sign, so deltas read `+` or `−` as the spec asks. */
export const MINUS = '−';
export function signed(value: number, digits: number): string {
  const rounded = Number(Math.abs(value).toFixed(digits));
  const sign = value < 0 && rounded !== 0 ? MINUS : '+';
  return `${sign}${rounded.toFixed(digits)}`;
}

/**
 * Size a canvas to a CSS box at the device's pixel ratio, so it is drawn sharp and never
 * scaled with CSS. Returns its 2D context, or null where there is none.
 */
export function fitCanvas(canvas: HTMLCanvasElement | null, width: number, height: number): CanvasRenderingContext2D | null {
  if (!canvas) return null;
  const ratio = globalThis.devicePixelRatio || 1;
  const w = Math.max(1, Math.round(width * ratio));
  const h = Math.max(1, Math.round(height * ratio));
  if (canvas.width !== w) canvas.width = w;
  if (canvas.height !== h) canvas.height = h;
  if (canvas.style.width !== `${width}px`) canvas.style.width = `${width}px`;
  if (canvas.style.height !== `${height}px`) canvas.style.height = `${height}px`;
  const ctx = canvas.getContext('2d');
  ctx?.setTransform(ratio, 0, 0, ratio, 0, 0);
  return ctx;
}

/** A palette colour for canvas drawing, which cannot use `var()`. */
export function tokenColor(el: Element | null, name: string, fallback: string): string {
  if (!el) return fallback;
  return getComputedStyle(el).getPropertyValue(name).trim() || fallback;
}
