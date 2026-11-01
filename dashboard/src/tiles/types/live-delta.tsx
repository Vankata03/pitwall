import { useRef, type CSSProperties } from 'react';
import { useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import { setData, showMessage, signed, TileMessage, useLifecycle, valueOf, write } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';
import { drawDelta } from '../live-delta/chart';
import { deltaGeometry } from '../live-delta/geometry';
import { DeltaLap, type Reference } from '../live-delta/lap';
import '../frame-tiles.css';
import '../live-delta/live-delta.css';

const declaration = {
  type: 'live-delta',
  name: 'Live delta',
  binding: 'follows-car',
  rate: 'frame',
  minSize: { w: 6, h: 3 },
  defaultSize: { w: 12, h: 6 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 280, minHeight: 170 },
    { density: 'full', minWidth: 600, minHeight: 240 },
  ],
  reads: [
    { source: 'frame', fields: ['deltaBest', 'deltaPrevious', 'lapDistance'] },
    { source: 'state', keys: ['session'] },
  ],
} as const;

const referenceOf = (settings: Record<string, unknown>) => (settings.reference === 'previous' ? 'previous' : 'best');

function LiveDelta({ car, settings, density, width, height }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const headline = useRef<HTMLSpanElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const history = useRef<{ key: string; lap: DeltaLap } | null>(null);
  const session = useSlow((s) => valueOf(s.state?.session));
  const lifecycle = useLifecycle();
  const reference = referenceOf(settings);
  const geometry = deltaGeometry(width, height, density);
  const chart = geometry.chart;
  const trackLength = session?.trackLength ?? 0;
  const sector2 = session?.sector2LapDistanceStart ?? 0;
  const sector3 = session?.sector3LapDistanceStart ?? 0;

  useFrameTile(
    declaration,
    car,
    ({ frame, car: index, rings, connected }) => {
      const field: Reference = reference === 'previous' ? 'deltaPrevious' : 'deltaBest';
      const state = index === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, index, field), connected);
      const ms = index === null ? 0 : frame[field][index]!;
      write(headline.current, state, signed(ms / 1000, 3));
      setData(headline.current, 'sign', state < ReadoutState.Restricted ? (ms < 0 ? 'ahead' : ms > 0 ? 'behind' : 'equal') : '');
      // A first lap has no reference, so absent is normal while connected: say why only when
      // the collector or the game is the reason.
      const message = state === ReadoutState.Absent ? wholeTileMessage([state], { lifecycle, connected }) : null;
      showMessage(root.current, message === 'No data' ? null : message);

      const key = `${index}:${reference}`;
      if (history.current?.key !== key) history.current = { key, lap: new DeltaLap() };
      if (rings) history.current.lap.ingest(rings, frame.epoch, field);
      if (chart) drawDelta(canvas.current, chart, history.current.lap, trackLength, [sector2, sector3], state !== ReadoutState.Live);
    },
    [reference, chart?.width, chart?.height, trackLength, sector2, sector3, lifecycle],
  );

  const style = { '--pw-ld-size': `${geometry.fontSize}px` } as CSSProperties;
  return (
    <div className="pw-frame-tile pw-live-delta" ref={root} data-density={density} data-arrangement={geometry.arrangement} style={style}>
      <div className="pw-frame-body">
        <div className="pw-live-delta-head">
          <span className="pw-label">{reference === 'previous' ? 'Previous lap' : 'Best lap'} · delta / s</span>
          <span ref={headline} className="pw-num pw-readout pw-live-delta-value" data-testid="live-delta-value" />
        </div>
        {chart ? <canvas ref={canvas} aria-label="Live delta in seconds against lap distance in metres" role="img" /> : null}
      </div>
      <TileMessage />
    </div>
  );
}

function Settings({ settings, onChange }: TileSettingsProps) {
  return (
    <label>
      Reference lap
      <select aria-label="Reference lap" value={referenceOf(settings)} onChange={(e) => onChange({ ...settings, reference: e.target.value })}>
        <option value="best">Best lap</option>
        <option value="previous">Previous lap</option>
      </select>
    </label>
  );
}

export default defineTileModule({ declaration, Component: LiveDelta, Settings });
