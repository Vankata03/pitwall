import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { useSlow } from '../../runtime/react';
import { effectiveState, frameReadoutState } from '../../runtime/readout';
import { CAR_COUNT } from '../../runtime/wire';
import { defineTileModule, type TileProps } from '../contract';
import { useFrameTile } from '../use-frame-tile';

type Point = [number, number];
interface TrackMap {
  trackId: number;
  trackLength: number;
  complete: boolean;
  outline: [number, number, number][];
  pit: Point[] | null;
}
type Bounds = { minX: number; minZ: number; maxX: number; maxZ: number };
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
      const h = Math.max(1, height - 40);
      const ratio = Math.max(1, window.devicePixelRatio || 1);
      if (el.width !== Math.round(width * ratio) || el.height !== Math.round(h * ratio)) {
        el.width = Math.round(width * ratio);
        el.height = Math.round(h * ratio);
      }
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
      ctx.clearRect(0, 0, width, h);
      const dots: { point: Point; state: number }[] = [];
      let restricted = 0;
      for (let i = 0; i < CAR_COUNT; i++) {
        const state = effectiveState(frameReadoutState(frame, i, 'worldX'), connected);
        if (state === 2) restricted++;
        const point: Point = [frame.worldX[i]!, frame.worldZ[i]!];
        if (state < 2 && finite(point)) dots.push({ point, state });
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
          Math.max(1, width - 40) / (b.maxX - b.minX),
          Math.max(1, h - 40) / (b.maxZ - b.minZ),
        ),
      );
      const project = ([x, z]: Point): Point => [
        width / 2 + (x - (b.minX + b.maxX) / 2) * scale,
        h / 2 + (z - (b.minZ + b.maxZ) / 2) * scale,
      ];
      ctx.lineWidth = 2;
      ctx.strokeStyle = '#9AA7AD';
      ctx.setLineDash([]);
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
      ctx.stroke();
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
        ctx.strokeStyle = '#F3F5F6';
        ctx.beginPath();
        ctx.moveTo(x - 4, y - 4);
        ctx.lineTo(x + 4, y + 4);
        ctx.stroke();
        ctx.fillStyle = '#F3F5F6';
        ctx.fillText(label, x + 5, y - 5);
      }
      for (const dot of dots) {
        const [x, y] = project(dot.point);
        ctx.fillStyle = dot.state === 1 ? '#9AA7AD' : '#FF9C35';
        ctx.beginPath();
        ctx.arc(x, y, 4, 0, Math.PI * 2);
        ctx.fill();
        if (dot.state === 1) {
          ctx.strokeStyle = '#F3F5F6';
          ctx.beginPath();
          ctx.arc(x, y, 6, 0, Math.PI * 2);
          ctx.stroke();
        }
      }
    },
    [width, height, map, outline, pit, identity, info],
  );
  return (
    <div style={{ height: '100%', overflow: 'hidden' }}>
      <div className="pw-label" style={{ padding: '4px 8px', minHeight: 20 }}>
        Track map {info && !map?.complete ? <span>Learning map</span> : null}{' '}
        {loaded?.identity === identity && loaded.failed ? <span>Map unavailable</span> : null}
      </div>
      <canvas
        ref={canvas}
        role="img"
        aria-label="Track map: raw car positions, start/finish, sectors and dashed pit lane"
        aria-describedby={summaryId}
        style={{ display: 'block', width, height: Math.max(1, height - 40) }}
      />
      <span ref={summary} id={summaryId} className="pw-label" style={{ padding: '0 8px' }}>
        —
      </span>
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
