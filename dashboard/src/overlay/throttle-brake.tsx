import { useEffect, useMemo, useRef } from 'react';
import type { ReadoutState } from '../runtime/wire';
import { useSlow } from '../runtime/react';
import { effectiveState, frameReadoutState, writeReadout } from '../runtime/readout';
import { defineTile } from '../runtime/tile';
import { participantsOf, playerCarOf, resolveFocus } from '../session/cars';
import { useFrameTile } from '../tiles/use-frame-tile';
import { InputHistory } from '../tiles/traces/window-plot';
import { useReferenceTrace } from '../tiles/traces/use-reference-trace';
import { inputTrace, type Polyline } from './input-trace';
import './overlay.css';

const declaration = defineTile({
  type: 'overlay-throttle-brake', name: 'Throttle/brake overlay', binding: 'follows-car', rate: 'frame',
  minSize: { w: 1, h: 1 }, defaultSize: { w: 1, h: 1 },
  densities: [{ density: 'compact', minWidth: 0, minHeight: 0 }, { density: 'regular', minWidth: 0, minHeight: 0 }],
  reads: [{ source: 'frame', fields: ['throttle', 'brake'] }, { source: 'state', keys: ['cars'] }],
});

/** Resize grips shown in move mode, as compass codes the collector understands. */
const EDGES = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw'] as const;

const FALLBACK = { throttle: '#56D9CA', brake: '#F55C72' };

function Bar({ name, label, fill, value }: { name: string; label: string; fill: React.Ref<HTMLElement>; value: React.Ref<HTMLElement> }) {
  return (
    <div className="pw-ov-bar" data-channel={name}>
      <b ref={value} className="pw-ov-value" data-testid={`overlay-${name}-value`} />
      <span className="pw-ov-track"><i ref={fill} data-testid={`overlay-${name}`} /></span>
      <span className="pw-ov-label">{label}</span>
    </div>
  );
}

/**
 * Draw each line as quadratic curves through the midpoints between samples, with the samples
 * as control points. That rounds off the steps between 60 Hz samples, and a curve never leaves
 * the hull of its points, so it cannot overshoot 0% or 100%.
 */
function stroke(context: CanvasRenderingContext2D, lines: Polyline[], colour: string) {
  context.strokeStyle = colour;
  for (const line of lines) {
    if (line.length < 2) continue;
    context.beginPath();
    context.moveTo(line[0]![0], line[0]![1]);
    for (let i = 1; i < line.length - 1; i++) {
      const [x, y] = line[i]!;
      const [nx, ny] = line[i + 1]!;
      context.quadraticCurveTo(x, y, (x + nx) / 2, (y + ny) / 2);
    }
    const [lx, ly] = line[line.length - 1]!;
    context.lineTo(lx, ly);
    context.stroke();
  }
}

/**
 * Compact throttle and brake readout for the player car, drawn on a transparent page so the
 * collector's native overlay window can float it over the game.
 */
export function ThrottleBrakeOverlay() {
  const wire = useSlow((s) => s.state);
  const epoch = useSlow((s) => s.epoch);
  const participants = useMemo(() => participantsOf(wire), [wire]);
  const car = resolveFocus({ participants, playerCar: playerCarOf(wire), focus: null });
  const carIndex = car.status === 'ok' || car.status === 'retired' ? car.index : null;
  // The dim second lap: the best lap, the same reference the inputs tile defaults to.
  const { traces, version, referenceLap } = useReferenceTrace(carIndex, 'best');
  const canvas = useRef<HTMLCanvasElement>(null);
  const throttleFill = useRef<HTMLElement>(null);
  const throttleValue = useRef<HTMLElement>(null);
  const brakeFill = useRef<HTMLElement>(null);
  const brakeValue = useRef<HTMLElement>(null);
  const legend = useRef<HTMLSpanElement>(null);
  const history = useRef<InputHistory | null>(null);
  if (!history.current) history.current = new InputHistory();

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.overlay = 'true';
    if (new URLSearchParams(globalThis.location.search).get('move') === '1') root.dataset.overlayMove = 'true';
    return () => { delete root.dataset.overlay; delete root.dataset.overlayMove; };
  }, []);

  useFrameTile(declaration, car, ({ frame, rings, car: index, connected }) => {
    const state = index === null ? 3 : effectiveState(frameReadoutState(frame, index, 'throttle'), connected);
    const showing = index !== null && (state === 0 || state === 1);
    const bar = (fill: HTMLElement | null, value: HTMLElement | null, percent: number) => {
      if (value) writeReadout(value, state as ReadoutState, `${percent}`);
      if (fill) {
        const height = `${showing ? Math.min(100, percent) : 0}%`;
        if (fill.style.height !== height) fill.style.height = height;
      }
    };
    bar(throttleFill.current, throttleValue.current, showing ? frame.throttle[index]! : 0);
    bar(brakeFill.current, brakeValue.current, showing ? frame.brake[index]! : 0);

    history.current!.update(rings, `${frame.epoch || epoch}:${index}`);
    const element = canvas.current;
    const context = element?.getContext('2d');
    if (!element || !context) return;
    const ratio = globalThis.devicePixelRatio || 1;
    const width = element.clientWidth, height = element.clientHeight;
    if (element.width !== Math.round(width * ratio) || element.height !== Math.round(height * ratio)) {
      element.width = Math.round(width * ratio);
      element.height = Math.round(height * ratio);
    }
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    if (!showing) return;
    const css = getComputedStyle(element);
    const reference = referenceLap === undefined ? null : traces.get(index, referenceLap) ?? null;
    const trace = inputTrace(history.current!.rings, width, height, reference);
    const throttle = css.getPropertyValue('--pw-energy').trim() || FALLBACK.throttle;
    const brake = css.getPropertyValue('--pw-brake').trim() || FALLBACK.brake;
    const alpha = state === 1 ? 0.5 : 1;
    context.lineJoin = 'round';
    // Reference lap first, thin and dim, so the live lap draws over it.
    context.lineWidth = 1.5;
    context.globalAlpha = alpha * 0.4;
    stroke(context, trace.reference.throttle, throttle);
    stroke(context, trace.reference.brake, brake);
    context.lineWidth = 3;
    context.globalAlpha = alpha;
    stroke(context, trace.throttle, throttle);
    stroke(context, trace.brake, brake);
    context.globalAlpha = 1;
    if (legend.current) legend.current.hidden = !reference;
  }, [epoch, version, referenceLap]);

  // In move mode the native window takes the mouse; wry's IPC asks it to start a move or resize.
  const send = (event: React.MouseEvent, message: string) => {
    if (event.button !== 0 || document.documentElement.dataset.overlayMove !== 'true') return;
    event.stopPropagation();
    (globalThis as { ipc?: { postMessage(message: string): void } }).ipc?.postMessage(message);
  };

  return (
    <main className="pw-ov" aria-label="Throttle and brake" onMouseDown={(event) => send(event, 'drag')}>
      <div className="pw-ov-handle" aria-hidden="true">Drag to move · edges to resize</div>
      {EDGES.map((edge) => <span key={edge} className="pw-ov-grip" data-edge={edge} aria-hidden="true"
        onMouseDown={(event) => send(event, `resize:${edge}`)} />)}
      <span ref={legend} className="pw-ov-legend" hidden>Dim: best lap</span>
      <canvas ref={canvas} className="pw-ov-trace" role="img" aria-label="Throttle and brake over the last 5 seconds" />
      <div className="pw-ov-bars">
        <Bar name="throttle" label="THR" fill={throttleFill} value={throttleValue} />
        <Bar name="brake" label="BRK" fill={brakeFill} value={brakeValue} />
      </div>
    </main>
  );
}
