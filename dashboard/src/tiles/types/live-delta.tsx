import { useRef } from 'react';
import { useRuntime, useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState, wholeTileMessage } from '../../runtime/readout';
import { ReadoutState } from '../../runtime/wire';
import { defineTileModule, type TileProps, type TileSettingsProps } from '../contract';
import { setData, showMessage, signed, TileMessage, useLifecycle, valueOf, write } from '../frame-kit';
import { useFrameTile } from '../use-frame-tile';
import { DeltaLap } from '../live-delta/lap';
import { drawDelta } from '../live-delta/chart';
import '../frame-tiles.css';
import '../live-delta/live-delta.css';

const declaration = {
  type: 'live-delta', name: 'Live delta', binding: 'follows-car', rate: 'frame',
  minSize: { w: 6, h: 3 }, defaultSize: { w: 12, h: 6 },
  densities: [
    { density: 'compact', minWidth: 0, minHeight: 0 },
    { density: 'regular', minWidth: 280, minHeight: 170 },
    { density: 'full', minWidth: 600, minHeight: 240 },
  ],
  reads: [{ source: 'frame', fields: ['deltaBest', 'deltaPrevious', 'lapDistance'] }, { source: 'state', keys: ['session'] }],
} as const;
const referenceOf = (settings: Record<string, unknown>) => settings.reference === 'previous' ? 'previous' : 'best';

function LiveDelta({ car, settings, density, width, height }: TileProps) {
  const root = useRef<HTMLDivElement>(null);
  const headline = useRef<HTMLSpanElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const history = useRef<{ key: string; lap: DeltaLap } | null>(null);
  const { stores } = useRuntime();
  const session = useSlow(s => s.state?.session);
  const epoch = useSlow(s => s.epoch);
  const lifecycle = useLifecycle();
  const reference = referenceOf(settings);
  useFrameTile(declaration, car, ({ frame, car: index, rings, connected }) => {
    const field = reference === 'previous' ? 'deltaPrevious' : 'deltaBest';
    const state = index === null ? ReadoutState.Absent : effectiveState(frameReadoutState(frame, index, field), connected);
    const value = index === null ? 0 : frame[field][index]!;
    write(headline.current, state, signed(value / 1000, 3));
    setData(headline.current, 'sign', state < 2 ? value < 0 ? 'ahead' : 'behind' : '');
    showMessage(root.current, wholeTileMessage([state], { lifecycle, connected }));
    const key = `${index}:${reference}:${epoch}`;
    if (history.current?.key !== key) history.current = { key, lap: new DeltaLap() };
    if (rings) history.current.lap.ingest(rings, frame.epoch, field);
    const info = valueOf(session);
    if (density !== 'compact') drawDelta(canvas.current, Math.max(1, width - 32), Math.max(1, height - 48 - (headline.current?.getBoundingClientRect().height || 56)),
      history.current.lap, info?.trackLength ?? 0, [info?.sector2LapDistanceStart ?? 0, info?.sector3LapDistanceStart ?? 0], state !== ReadoutState.Live);
  }, [reference, density, width, height, session, lifecycle, epoch, stores]);
  return <div className="pw-frame-tile pw-live-delta" ref={root} data-density={density}>
    <div className="pw-frame-body">
      <span className="pw-label">{reference === 'previous' ? 'Previous lap' : 'Best lap'} · delta / s</span>
      <span ref={headline} className="pw-num pw-readout pw-live-delta-value" data-testid="live-delta-value" />
      {density !== 'compact' ? <canvas ref={canvas} aria-label="Live delta in seconds against lap distance in metres" role="img" /> : null}
    </div><TileMessage />
  </div>;
}
function Settings({ settings, onChange }: TileSettingsProps) {
  return <label>Reference lap<select aria-label="Reference lap" value={referenceOf(settings)} onChange={e => onChange({ ...settings, reference: e.target.value })}>
    <option value="best">Best lap</option><option value="previous">Previous lap</option>
  </select></label>;
}
export default defineTileModule({ declaration, Component: LiveDelta, Settings });
