import { useContext, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState } from '../../runtime/readout';
import { CAR_COUNT } from '../../runtime/wire';
import { trackName } from '../../session/names';
import { defineTileModule, type TileProps } from '../contract';
import '../frame-tiles.css';
import { TileTitle, tokenColor } from '../frame-kit';
import { TowerFocus } from './timing-tower';
import { useFrameTile } from '../use-frame-tile';
import { carLabel, participantsOf } from '../../session/cars';

type Point = [number, number];
interface TrackMap {
  trackId: number;
  trackLength: number;
  complete: boolean;
  outline: [number, number, number][];
  pit: Point[] | null;
}
type Bounds = { minX: number; minZ: number; maxX: number; maxZ: number };
/** Height the name row takes from the tile. The title floats over the map's empty corner. */
const CHROME_HEIGHT = 40;
/** Room around the track for the markers and their numbers. */
const MAP_INSET = 20;
const LABEL_WIDTH = 16;
const LABEL_HEIGHT = 12;
const finite = (p: readonly number[]) => p.every(Number.isFinite);

function TrackMapTile({ width, height }: TileProps) {
  const summaryId = useId();
  const slow = useSlow((s) => s);
  const session = slow.state?.session;
  const info = session && 'value' in session ? session.value : null;
  const trackId = info?.trackId;
  const trackLength = info?.trackLength;
  const revision = slow.state?.context?.revisions?.tracks;
  const identity = `${slow.epoch}:${trackId}:${trackLength}`;
  const [loaded, setLoaded] = useState<{
    identity: string;
    map: TrackMap | null;
    failed: boolean;
  } | null>(null);
  const map = loaded?.identity === identity ? loaded.map : null;
  const outline = useMemo(
    () =>
      (map?.outline ?? [])
        .filter((p) => p.length === 3 && finite(p))
        .slice()
        .sort((a, b) => a[0] - b[0]),
    [map],
  );
  const pit = useMemo(() => (map?.pit ?? []).filter((p) => finite(p)), [map]);
  const canvas = useRef<HTMLCanvasElement>(null);
  const cars = slow.state?.cars;
  const focusCar = useContext(TowerFocus) ?? slow.state?.playerCar ?? null;
  const positions = useMemo(
    () =>
      (cars ?? []).map((car) => {
        const lap = car?.lap;
        return lap && 'value' in lap && lap.value.car_position > 0 ? lap.value.car_position : null;
      }),
    [cars],
  );
  // Each dot wears its team's livery colour, as the tower's team marker does.
  const colours = useMemo(
    () =>
      (cars ?? []).map((car) => {
        const participant = car?.participant;
        const colour = participant && 'value' in participant ? participant.value.livery_colour?.[0] : undefined;
        return colour ? `rgb(${colour.red},${colour.green},${colour.blue})` : null;
      }),
    [cars],
  );
  // The focus car's name rides beside its dot, as on a broadcast map.
  const focusName = useMemo(() => {
    const p = participantsOf(slow.state).find((c) => c.index === focusCar);
    return p ? carLabel(p).toUpperCase() : null;
  }, [slow.state, focusCar]);
  const summary = useRef<HTMLSpanElement>(null);
  const fitting = useRef<{ identity: string; bounds: Bounds | null }>({
    identity,
    bounds: null,
  });

  useEffect(() => {
    if (trackId === undefined || !trackLength) return;
    const controller = new AbortController();
    let active = true;
    fetch(`/api/tracks/${trackId}/${trackLength}`, {
      signal: controller.signal,
    })
      .then(async (response) => {
        if (response.status === 404) return null;
        if (!response.ok) throw new Error('Track map request failed');
        const result: TrackMap = await response.json();
        if (
          result.trackId !== trackId ||
          result.trackLength !== trackLength ||
          !Array.isArray(result.outline)
        )
          throw new Error('Track map identity mismatch');
        return result;
      })
      .then((result) => {
        if (active) setLoaded({ identity, map: result, failed: false });
      })
      .catch(() => {
        if (active) setLoaded({ identity, map: null, failed: true });
      });
    return () => {
      active = false;
      controller.abort();
    };
  }, [trackId, trackLength, revision, identity, slow.connection]);

  useFrameTile(
    tile.declaration,
    null,
    ({ frame, connected }) => {
      const el = canvas.current;
      const ctx = el?.getContext('2d');
      if (!el || !ctx || width <= 0 || height <= 0) return;
      const w = width;
      const h = Math.max(1, height - CHROME_HEIGHT);
      const ratio = Math.max(1, window.devicePixelRatio || 1);
      if (el.width !== Math.round(w * ratio) || el.height !== Math.round(h * ratio)) {
        el.width = Math.round(w * ratio);
        el.height = Math.round(h * ratio);
      }
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const dots: { point: Point; state: number; index: number }[] = [];
      let restricted = 0;
      for (let i = 0; i < CAR_COUNT; i++) {
        const state = effectiveState(frameReadoutState(frame, i, 'worldX'), connected);
        if (state === 2) restricted++;
        const point: Point = [frame.worldX[i]!, frame.worldZ[i]!];
        if (state < 2 && finite(point)) dots.push({ point, state, index: i });
      }
      if (summary.current)
        summary.current.textContent = dots.length
          ? `${dots.length} car${dots.length === 1 ? '' : 's'}${dots.some((d) => d.state === 1) ? ' · stale ◷' : ''}${restricted ? ` · ${restricted} restricted` : ''}`
          : restricted
            ? 'Restricted'
            : '—';
      const points: Point[] = [
        ...outline.map((p) => [p[1], p[2]] as Point),
        ...pit,
        ...dots.map((d) => d.point),
      ];
      if (fitting.current.identity !== identity) fitting.current = { identity, bounds: null };
      if (!points.length) return;
      // Expand in 100 m steps. Moving dots inside this envelope never change the fit.
      let bounds = fitting.current.bounds;
      for (const [x, z] of points) {
        if (!bounds)
          bounds = {
            minX: Math.floor(x / 100) * 100,
            minZ: Math.floor(z / 100) * 100,
            maxX: Math.floor(x / 100) * 100 + 100,
            maxZ: Math.floor(z / 100) * 100 + 100,
          };
        if (x < bounds.minX) bounds.minX = Math.floor(x / 100) * 100;
        if (z < bounds.minZ) bounds.minZ = Math.floor(z / 100) * 100;
        if (x > bounds.maxX) bounds.maxX = Math.ceil(x / 100) * 100;
        if (z > bounds.maxZ) bounds.maxZ = Math.ceil(z / 100) * 100;
      }
      fitting.current.bounds = bounds;
      const b = bounds!;
      const scale = Math.max(
        0.001,
        Math.min(
          Math.max(1, w - 2 * MAP_INSET) / (b.maxX - b.minX),
          Math.max(1, h - 2 * MAP_INSET) / (b.maxZ - b.minZ),
        ),
      );
      const project = ([x, z]: Point): Point => [
        w / 2 + (x - (b.minX + b.maxX) / 2) * scale,
        h / 2 + (z - (b.minZ + b.maxZ) / 2) * scale,
      ];
      const track = tokenColor(el, '--pw-track', '#6B8496');
      const muted = tokenColor(el, '--pw-text-muted', '#9AA7AD');
      const ink = tokenColor(el, '--pw-text', '#F3F5F6');
      const accent = tokenColor(el, '--pw-accent', '#FF9C35');
      const field = tokenColor(el, '--pw-energy', '#56D9CA');
      const casing = tokenColor(el, '--pw-page-bg', '#121517');
      const edge = tokenColor(el, '--pw-line', '#2E373C');
      const road = Math.max(4, Math.min(10, Math.min(w, h) / 22));
      const tracePath = () => {
        ctx.beginPath();
        outline.forEach((p, i) => {
          const point = project([p[1], p[2]]);
          if (i && (p[0] - outline[i - 1]![0]) * 8 <= 40) ctx.lineTo(...point);
          else ctx.moveTo(...point);
        });
        const first = outline[0],
          last = outline.at(-1);
        if (map?.complete && first && last && trackLength! - (last[0] - first[0]) * 8 <= 40)
          ctx.lineTo(...project([first[1], first[2]]));
      };
      // A road: a dark casing, a lighter kerb edge, the tarmac, and a faint centre line.
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.setLineDash([]);
      tracePath();
      ctx.strokeStyle = casing;
      ctx.lineWidth = road + 6;
      ctx.stroke();
      ctx.strokeStyle = edge;
      ctx.lineWidth = road + 2;
      ctx.stroke();
      ctx.strokeStyle = track;
      ctx.lineWidth = road;
      ctx.stroke();
      ctx.globalAlpha = 0.18;
      ctx.strokeStyle = ink;
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.globalAlpha = 1;
      ctx.lineWidth = 2;
      ctx.lineCap = 'butt';
      ctx.strokeStyle = muted;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      pit.forEach((p, i) => (i ? ctx.lineTo(...project(p)) : ctx.moveTo(...project(p))));
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.font = '11px Oxanium, sans-serif';
      ctx.textAlign = 'left';
      for (const [distance, label] of [
        [0, 'S/F'],
        [info?.sector2LapDistanceStart, 'S2'],
        [info?.sector3LapDistanceStart, 'S3'],
      ] as const) {
        if (distance === undefined || (label !== 'S/F' && distance <= 0)) continue;
        let marker: Point | null = null;
        for (let i = 0; i < outline.length; i++) {
          const p = outline[i]!;
          if (p[0] * 8 === distance) {
            marker = [p[1], p[2]];
            break;
          }
          const next = outline[i + 1];
          if (next && p[0] * 8 < distance && next[0] * 8 > distance && (next[0] - p[0]) * 8 <= 40) {
            const fraction = (distance - p[0] * 8) / ((next[0] - p[0]) * 8);
            marker = [p[1] + (next[1] - p[1]) * fraction, p[2] + (next[2] - p[2]) * fraction];
            break;
          }
        }
        if (!marker) continue;
        const [x, y] = project(marker);
        ctx.strokeStyle = ink;
        ctx.beginPath();
        ctx.moveTo(x - 4, y - 4);
        ctx.lineTo(x + 4, y + 4);
        ctx.stroke();
        ctx.fillStyle = ink;
        ctx.fillText(label, x + 5, y - 5);
      }
      // Numbered by race position. The focus car goes last, so it is never hidden.
      const ordered = [...dots].sort((a, b) => Number(a.index === focusCar) - Number(b.index === focusCar));
      const taken: Point[] = [];
      for (const dot of ordered) {
        const [x, y] = project(dot.point);
        const focused = dot.index === focusCar;
        ctx.fillStyle = dot.state === 1 ? muted : (colours[dot.index] ?? (focused ? accent : field));
        ctx.beginPath();
        ctx.arc(x, y, focused && dot.state !== 1 ? 6 : 4, 0, Math.PI * 2);
        ctx.fill();
        // A dark rim keeps dark liveries visible; the focus car gets an accent ring.
        if (dot.state !== 1) {
          ctx.lineWidth = focused ? 2 : 1;
          ctx.strokeStyle = focused ? accent : casing;
          ctx.stroke();
        }
        if (dot.state === 1) {
          ctx.lineWidth = 2;
          ctx.strokeStyle = ink;
          ctx.beginPath();
          ctx.arc(x, y, 6, 0, Math.PI * 2);
          ctx.stroke();
        }
      }
      for (const dot of [...ordered].reverse()) {
        const position = positions[dot.index];
        if (!position) continue;
        const [x, y] = project(dot.point);
        // A crowded field drops numbers rather than stacking them.
        if (taken.some(([a, b]) => Math.abs(a - x) < LABEL_WIDTH && Math.abs(b - y) < LABEL_HEIGHT)) continue;
        taken.push([x, y]);
        ctx.fillStyle = dot.index === focusCar ? accent : ink;
        ctx.fillText(String(position).padStart(2, '0'), x + 7, y - 7);
      }
      const focusDot = dots.find((d) => d.index === focusCar);
      if (focusDot && focusName) {
        const [x, y] = project(focusDot.point);
        ctx.font = '600 12px Oxanium, sans-serif';
        ctx.textAlign = x > w * 0.6 ? 'right' : 'left';
        const tx = x > w * 0.6 ? x - 10 : x + 10;
        ctx.lineJoin = 'round';
        ctx.lineWidth = 4;
        ctx.strokeStyle = casing;
        ctx.strokeText(focusName, tx, y + 16);
        ctx.fillStyle = accent;
        ctx.fillText(focusName, tx, y + 16);
      }
    },
    [width, height, map, outline, pit, identity, info, positions, colours, focusCar, focusName],
  );
  return (
    <div className="pw-tm">
      <TileTitle
        note={
          <>
            {info && !map?.complete ? <span>Learning map</span> : null}
            {loaded?.identity === identity && loaded.failed ? <span>Map unavailable</span> : null}
          </>
        }
      >
        Circuit position
      </TileTitle>
      <canvas
        ref={canvas}
        className="pw-tm-map"
        role="img"
        aria-label="Track map: raw car positions, start/finish, sectors and dashed pit lane"
        aria-describedby={summaryId}
        style={{ width, height: Math.max(1, height - CHROME_HEIGHT) }}
      />
      <div className="pw-tm-foot">
        <span className="pw-tm-name" data-testid="track-name">
          {trackName(trackId) ?? 'Track'}
        </span>
        <span className="pw-tm-facts">
          <span ref={summary} id={summaryId} className="pw-label">
            —
          </span>
          {trackLength ? <span className="pw-label pw-num">{(trackLength / 1000).toFixed(3)} km</span> : null}
        </span>
      </div>
    </div>
  );
}
const tile = defineTileModule({
  declaration: {
    type: 'track-map',
    name: 'Track map',
    binding: 'session-wide',
    rate: 'frame',
    minSize: { w: 4, h: 4 },
    defaultSize: { w: 6, h: 6 },
    densities: [
      { density: 'compact', minWidth: 0, minHeight: 0 },
      { density: 'regular', minWidth: 240, minHeight: 160 },
    ],
    reads: [
      { source: 'frame', fields: ['worldX', 'worldZ'] },
      { source: 'state', keys: ['session', 'context'] },
    ],
  },
  Component: TrackMapTile,
});
export default tile;
