import { useRef } from 'react';
import { effectiveState, frameReadoutState, readoutStateName, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import {
  grouped,
  setData,
  showMessage,
  slowState,
  SlowReadout,
  StateReadout,
  TileMessage,
  useCarState,
  useConnected,
  useLifecycle,
  valueOf,
  write,
} from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';

/** Segments in the battery. */
const SEGMENTS = 20;
/** The game's ERS deploy mode for overtake. A 2026 car's boost reads as this while overtake is not active. */
const DEPLOY_OVERTAKE = 3;
const MODES = ['None', 'Medium', 'Hotlap', 'Overtake'];

const declaration = {
  type: 'ers',
  name: 'ERS',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 4, h: 2 },
  defaultSize: { w: 6, h: 4 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 240, minHeight: 100 },
    { density: 'full', minWidth: 340, minHeight: 160 },
  ],
  reads: [
    { source: 'frame', fields: ['ersPercent'] },
    { source: 'state', keys: ['cars'] },
  ],
} as const;

const kj = (joules: number): string => `${grouped(joules / 1000)} kJ`;
const modeName = (mode: number): string => MODES[mode] ?? `Mode ${mode}`;

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="pw-stat">
      <span className="pw-label">{label}</span>
      <span className="pw-num pw-value">{children}</span>
    </div>
  );
}

function Ers({ car, density }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const percentEl = useRef<HTMLSpanElement>(null);
  const battery = useRef<HTMLDivElement>(null);
  const carState = useCarState(car);
  const connected = useConnected();
  const lifecycle = useLifecycle();

  const ersReadout = carState?.ers;
  const aeroReadout = carState?.aero;
  const is2026 = carState?.regulations === 'Regulations2026';
  const showRows = density !== 'compact';
  const show2026 = density === 'full' && is2026;

  // What the slow rows currently show, for the whole-tile rule.
  const slowStates = showRows ? [slowState(ersReadout, connected)] : [];
  if (show2026) slowStates.push(slowState(aeroReadout, connected));

  useFrameTile(
    declaration,
    car,
    ({ frame, car: index, connected: online }) => {
      const state =
        index === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, index, 'ersPercent'), online);
      const live = index !== null && (state === ReadoutState.Live || state === ReadoutState.Stale);
      const percent = live ? frame.ersPercent[index]! : 0;
      write(percentEl.current, state, `${percent}%`);
      const lit = live ? Math.round((Math.min(100, percent) / 100) * SEGMENTS) : 0;
      const bar = battery.current;
      if (bar) {
        setData(bar, 'state', readoutStateName(state));
        if (bar.dataset.lit !== String(lit)) {
          bar.dataset.lit = String(lit);
          for (let i = 0; i < bar.children.length; i++) (bar.children[i] as HTMLElement).dataset.on = String(i < lit);
        }
      }
      const states = [state, ...slowStates.map((s) => (s === ReadoutState.Live && !online ? ReadoutState.Stale : s))];
      showMessage(root.current, wholeTileMessage(states, { lifecycle, connected: online }));
    },
    [carState, density, lifecycle, connected],
  );

  const ers = valueOf(ersReadout);
  const aero = valueOf(aeroReadout);
  const overtakeActive = aero?.overtake_active === 1;
  const overtake = aero ? (overtakeActive ? 'Active' : aero.overtake_available === 1 ? 'Available' : 'Off') : '';
  const boost = ers ? (ers.deploy_mode === DEPLOY_OVERTAKE && !overtakeActive ? 'On' : 'Off') : '';
  const limit = ers?.harvest_limit_per_lap;

  return (
    <div className="pw-frame-tile pw-ers" ref={root} data-density={density}>
      <div className="pw-frame-body">
        <div className="pw-ers-head">
          <span className="pw-num pw-readout pw-big pw-ers-percent" ref={percentEl} data-testid="ers-percent" />
          <div className="pw-battery" ref={battery} data-testid="ers-battery" data-segments={SEGMENTS} data-lit="0" aria-hidden="true">
            {Array.from({ length: SEGMENTS }, (_, i) => (
              <span key={i} data-on="false" />
            ))}
          </div>
        </div>
        {showRows ? (
          <div className="pw-ers-rows">
            <Row label="Deploy mode">
              <SlowReadout readout={ersReadout} connected={connected} format={(e) => modeName(e.deploy_mode)} testId="ers-mode" />
            </Row>
            <Row label="MGU-K harvest">
              <SlowReadout readout={ersReadout} connected={connected} format={(e) => kj(e.harvested_this_lap_mguk)} testId="ers-mgu-k" />
            </Row>
            <Row label="MGU-H harvest">
              <SlowReadout readout={ersReadout} connected={connected} format={(e) => kj(e.harvested_this_lap_mguh)} testId="ers-mgu-h" />
            </Row>
            <Row label="Deployed">
              <SlowReadout readout={ersReadout} connected={connected} format={(e) => kj(e.deployed_this_lap)} testId="ers-deployed" />
            </Row>
            {show2026 ? (
              <>
                <Row label="Overtake">
                  <SlowReadout readout={aeroReadout} connected={connected} format={() => overtake} testId="ers-overtake" />
                </Row>
                <Row label="Boost">
                  <StateReadout state={Math.max(slowState(ersReadout, connected), slowState(aeroReadout, connected)) as ReadoutState} text={boost} testId="ers-boost" />
                </Row>
                <Row label="Harvest limit">
                  {/* Assumed to be in joules like the other ERS fields, until a capture confirms it. */}
                  {limit === null || limit === undefined ? (
                    <StateReadout state={ersReadout?.state === 'restricted' ? ReadoutState.Restricted : ReadoutState.Absent} text="" testId="ers-harvest-limit" />
                  ) : (
                    <SlowReadout readout={ersReadout} connected={connected} format={() => kj(limit)} testId="ers-harvest-limit" />
                  )}
                </Row>
              </>
            ) : null}
          </div>
        ) : null}
      </div>
      <TileMessage />
    </div>
  );
}

export default defineTileModule({ declaration, Component: Ers });
