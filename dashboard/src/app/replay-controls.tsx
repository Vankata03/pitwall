import { useRef, useState } from 'react';
import type { ReplayState } from '../generated/wire/ReplayState';

/** Keep the thumb under local control; rebuilding replay happens only when input is committed. */
function ReplayPosition({ replay, onSeek }: { replay: ReplayState; onSeek: (position: number) => Promise<boolean> }) {
  const [draft, setDraft] = useState<number | null>(null);
  const dirty = useRef(false);
  const commit = (value: string) => {
    if (!dirty.current) return;
    dirty.current = false;
    void onSeek(Number(value)).then(ok => { if (!ok) setDraft(null); });
  };
  const position = draft ?? replay.position;
  return <>
    <label>Replay position<input type="range" min={0} max={replay.duration} step={0.1} value={position}
      onChange={e => { dirty.current = true; setDraft(Number(e.target.value)); }}
      onPointerDown={e => e.currentTarget.setPointerCapture?.(e.pointerId)}
      onPointerUp={e => commit(e.currentTarget.value)}
      onKeyUp={e => { if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(e.key)) commit(e.currentTarget.value); }}
      onBlur={e => commit(e.currentTarget.value)}
      onPointerCancel={() => { dirty.current = false; setDraft(null); }}
    /></label>
    <span>{position.toFixed(1)} / {replay.duration.toFixed(1)} s</span>
  </>;
}

/** Transport is shared by every screen; source start and exit remain host-only. */
export function ReplayControls({ replay, car, epoch }: { replay: ReplayState | null | undefined; car: number; epoch: number | null }) {
  const [lap, setLap] = useState(1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!replay) return null;
  const loading = replay.loadProgress < 1;
  const ended = replay.duration > 0 && replay.position >= replay.duration;
  async function post(route: string, body: unknown) {
    const response = await fetch(`/api/replay/${route}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
    if (!response.ok) throw new Error(`Replay ${route} failed: ${response.status}`);
  }
  async function run(action: () => Promise<void>) {
    setBusy(true); setError(null);
    try { await action(); return true; } catch (e) { setError(String(e)); return false; } finally { setBusy(false); }
  }
  return <section aria-label="Replay transport" className="pw-replay-controls">
    <p role="status">{replay.banner}</p>
    {replay.backgroundLive ? <p>{replay.backgroundLive}</p> : null}
    {loading ? <label>Loading replay <progress max={1} value={replay.loadProgress} /></label> : null}
    <fieldset disabled={busy || loading}>
      <legend>Replay controls</legend>
      <button onClick={() => void run(() => post('pause', { paused: !replay.paused }))}>{replay.paused ? 'Resume replay' : 'Pause replay'}</button>
      <label>Replay speed<select value={replay.speed} onChange={(e) => void run(() => post('speed', { speed: Number(e.target.value) }))}>
        {[0.5, 1, 2, 4].map((speed) => <option key={speed} value={speed}>{speed}x</option>)}
      </select></label>
      <ReplayPosition key={epoch} replay={replay} onSeek={position => run(() => post('seek', { position }))} />
      <form onSubmit={(e) => { e.preventDefault(); void run(() => post('lap', { car, lap })); }}>
        <label>Replay lap<input type="number" min={1} max={255} required value={lap} onChange={(e) => setLap(Number(e.target.value))} /></label>
        <button type="submit">Jump to lap</button>
      </form>
      {ended ? <button onClick={() => void run(async () => { await post('seek', { position: 0 }); await post('pause', { paused: false }); })}>Restart replay</button> : null}
    </fieldset>
    {ended ? <a href="/status">Exit replay on host</a> : null}
    {error ? <p role="alert">{error}</p> : null}
  </section>;
}
