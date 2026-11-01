import { useEffect, useState } from 'react';
import type { CollectorStatus } from '../generated/wire/CollectorStatus';
import type { ReplayState } from '../generated/wire/ReplayState';
import './status.css';

interface Config {
  udp: { port: number }; http: { port: number; bind: string };
  forward: { enabled: boolean; targets: string[] };
  recording: { enabled: boolean; size_cap_gb: number };
  app: { start_with_windows: boolean };
}
interface Diagnostics {
  udpPort: number; reloadError?: string | null; host: boolean; collector: CollectorStatus; urls: string[]; missingStreamLayouts: string[];
  track: [number, number] | null; cpuPercent: number | null; memoryBytes: number | null;
  processingP95Ms: number | null; processingP99Ms: number | null; recorderDropped: number;
  replay?: ReplayState | null;
}
interface Recording { name: string; size: number; sidecar: { pinned: boolean; track: string | null; duration_us: number; laps: number } }
async function request<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(path, { method, ...(body === undefined ? {} : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }) });
  if (!response.ok) throw new Error(`${method} ${path}: ${response.status}`);
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
export function StatusPage() {
  const [status, setStatus] = useState<Diagnostics | null>(null);
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [config, setConfig] = useState<Config | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  async function refresh() {
    const [next, files] = await Promise.all([request<Diagnostics>('/api/status'), request<Recording[]>('/api/recordings')]);
    setStatus(next); setRecordings(files);
    return next;
  }
  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const next = await refresh();
        if (next.host) { const settings = await request<Config>('/api/settings'); if (active) setConfig(settings); }
      } catch (e) { if (active) setError(String(e)); }
    };
    void load();
    const timer = setInterval(() => { void refresh().catch((e) => { if (active) setError(String(e)); }); }, 2000);
    return () => { active = false; clearInterval(timer); };
  }, []);
  async function action(path: string, body: unknown = {}, method = 'POST') {
    setError(null); setBusy(true);
    try { await request(path, method, body); await refresh(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  async function save() {
    if (!config || !status) return;
    const current = await request<Config>('/api/settings');
    if ((current.http.port !== config.http.port || current.http.bind !== config.http.bind) && !window.confirm('Changing the HTTP port or bind address disconnects every screen. Continue and redirect this page?')) return;
    setError(null); setBusy(true); setSaved(false);
    try {
      const result = await request<{ redirect: string | null }>('/api/settings', 'PUT', config);
      setSaved(true);
      if (result.redirect) { setTimeout(() => window.location.assign(result.redirect!), 1500); }
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  const c = status?.collector;
  const measure = (value: number | null | undefined, suffix: string) => value == null ? 'Unavailable' : `${value.toFixed(2)} ${suffix}`;
  return <main className="pw-status">
    <header><a href="/">← Dashboard</a><h1>Collector status</h1></header>
    {error ? <p role="alert">Collector request failed: {error}. Retrying status automatically.</p> : null}
    {!status ? <p>Connecting to collector…</p> : <>
      {status.reloadError ? <p role="alert">Settings reload rejected; active settings retained: {status.reloadError}</p> : null}
      <section aria-labelledby="connection"><h2 id="connection">Connection</h2>
        <p>{c && c.packetRateHz > 0 ? `Receiving ${c.packetRateHz} Hz` : 'Waiting for game'}</p>
        {c && c.packetRateHz > 0 && c.packetRateHz < 60 ? <p>Measured rate is below 60 Hz. Set UDP Send Rate to 60 Hz.</p> : null}
        <dl><dt>Dropped packets</dt><dd>{c?.dropped}</dd><dt>Recorder queue drops</dt><dd>{status.recorderDropped}</dd><dt>UID 0</dt><dd>{c?.uidZero}</dd><dt>Closed UID</dt><dd>{c?.closedUid}</dd><dt>Parse errors</dt><dd>{c?.parseErrors}</dd><dt>Forwarding errors</dt><dd>{c?.forwardingErrors}</dd>
          <dt>CPU (one logical core)</dt><dd>{measure(status.cpuPercent, '%')}</dd><dt>Resident memory</dt><dd>{measure(status.memoryBytes == null ? null : status.memoryBytes / 1048576, 'MiB')}</dd>
          <dt>Collector processing p95</dt><dd>{measure(status.processingP95Ms, 'ms')}</dd><dt>Collector processing p99</dt><dd>{measure(status.processingP99Ms, 'ms')}</dd></dl>
        <p>Processing measures each UDP poll and model update batch. Datagram-to-draw latency is unmeasured.</p>
        <h3>Missing Stream view layouts</h3>{status.missingStreamLayouts.length ? <ul>{status.missingStreamLayouts.map((name) => <li key={name}>{name}</li>)}</ul> : <p>None reported.</p>}
      </section>
      <section><h2>Connect another screen</h2><img src="/api/status/qr" width="192" height="192" alt="QR code for the first candidate URL" />
        <ul>{status.urls.map((url) => <li key={url}><a href={url}>{url}</a></li>)}</ul>
        <p>The QR code uses the default-route interface when available. pitwall.local is a best-effort alternative.</p>
        <p>Viewing is open over plain HTTP, without a login. Use a trusted LAN. Settings and host controls require a loopback connection: open localhost on the collector computer.</p>
      </section>
      <section><h2>Game setup checklist</h2><ul>
        <li>Enable UDP Telemetry in the F1® 25 game; it is off by default.</li><li>Set UDP Format to 2025 or 2026.</li><li>Set UDP Port to {status.udpPort ?? 20777}, matching collector settings.</li><li>Set UDP IP to the collector’s LAN IP, or 127.0.0.1 on the same computer. Broadcast is an alternative to forwarding.</li><li>Set UDP Send Rate to 60 Hz; 20–60 Hz is supported.</li>
      </ul>
        {c?.lastFormat != null && ![2025, 2026].includes(c.lastFormat) ? <p role="status">Unsupported telemetry format {c.lastFormat}. Set UDP Format to 2025 or 2026. Unsupported packets are neither parsed nor recorded.</p> : <p>Received format: {c?.lastFormat ?? 'none'}. Both 2025 and 2026 are accepted.</p>}
        <p>Allow pitwall through the first-launch Windows Firewall prompt on Private networks. Public-network blocking can prevent LAN screens and telemetry from connecting.</p>
        <p>The portable binary is unsigned. Windows SmartScreen may warn; use More info → Run anyway only for a build you trust. On macOS, Gatekeeper may require Open Anyway in System Settings → Privacy &amp; Security.</p>
        <p>Plain HTTP cannot use Wake Lock. On iPad, set Settings → Display &amp; Brightness → Auto-Lock → Never while using the dashboard, then restore your preference.</p>
      </section>
      <section><h2>Recordings and replay</h2>
        {status.replay ? <p role="status">{status.replay.banner} · {Math.round(status.replay.loadProgress * 100)}% loaded · {status.replay.position.toFixed(1)} s · {status.replay.paused ? 'Paused' : 'Playing'}{status.replay.backgroundLive ? ` · ${status.replay.backgroundLive}` : ''}</p> : <p>Live source</p>}
        {status.host ? <div className="pw-status-actions"><button disabled={busy} onClick={() => void action('/api/recordings/rescan')}>Rescan recordings</button><button disabled={busy} onClick={() => void action('/api/recordings/open-folder')}>Open recordings folder</button><button disabled={busy} onClick={() => void action('/api/replay/stop')}>Stop replay</button></div> : null}
        {recordings.length === 0 ? <p>No recordings.</p> : <ul>{recordings.map((file) => <li key={file.name}><strong>{file.name}</strong> · {file.sidecar.track ?? 'Unknown track'} · {(file.sidecar.duration_us / 1000000).toFixed(0)} s · {file.sidecar.laps} laps · {(file.size / 1048576).toFixed(1)} MiB · {file.sidecar.pinned ? 'Pinned' : 'Unpinned'}
          {status.host ? <div className="pw-status-actions"><button disabled={busy} onClick={() => void action('/api/replay/start', { name: file.name })}>Start replay</button><button disabled={busy} onClick={() => void action(`/api/recordings/${encodeURIComponent(file.name)}/pin`, { pinned: !file.sidecar.pinned })}>{file.sidecar.pinned ? 'Unpin' : 'Pin'}</button><button disabled={busy} onClick={() => { if (window.confirm(`Delete ${file.name}? This cannot be undone.`)) void action(`/api/recordings/${encodeURIComponent(file.name)}`, {}, 'DELETE'); }}>Delete</button></div> : null}</li>)}</ul>}
      </section>
      {status.host ? <section><h2>Host settings</h2>
        {config ? <form onSubmit={(event) => { event.preventDefault(); void save().catch((e) => setError(String(e))); }}>
          <label>UDP port<input type="number" min="1" max="65535" required value={config.udp.port} onChange={(e) => setConfig({ ...config, udp: { port: Number(e.target.value) } })} /></label>
          <label>HTTP port<input type="number" min="1" max="65535" required value={config.http.port} onChange={(e) => setConfig({ ...config, http: { ...config.http, port: Number(e.target.value) } })} /></label>
          <label>HTTP bind address<input required value={config.http.bind} onChange={(e) => setConfig({ ...config, http: { ...config.http, bind: e.target.value } })} /></label>
          <p>HTTP changes restart the listener and redirect this page. Other screens disconnect and need the new URL.</p>
          <label><input type="checkbox" checked={config.forward.enabled} onChange={(e) => setConfig({ ...config, forward: { ...config.forward, enabled: e.target.checked } })} />Forward live telemetry</label>
          <label>Forward targets (one host:port per line)<textarea value={config.forward.targets.join('\n')} onChange={(e) => setConfig({ ...config, forward: { ...config.forward, targets: e.target.value.split('\n').filter(Boolean) } })} /></label>
          <label><input type="checkbox" checked={config.recording.enabled} onChange={(e) => setConfig({ ...config, recording: { ...config.recording, enabled: e.target.checked } })} />Record live telemetry</label>
          <label>Recording size cap (GB)<input type="number" min="1" required value={config.recording.size_cap_gb} onChange={(e) => setConfig({ ...config, recording: { ...config.recording, size_cap_gb: Number(e.target.value) } })} /></label>
          <label><input type="checkbox" checked={config.app.start_with_windows} onChange={(e) => setConfig({ ...config, app: { start_with_windows: e.target.checked } })} />Start with Windows</label>
          <p>Settings persist in config.toml. CLI flags override saved values: --console, --udp-port, --http-port, --bind, --config.</p>
          <button disabled={busy} type="submit">Save settings</button>{saved ? <p role="status">Settings saved. Live reload applies valid changes; CLI overrides remain effective.</p> : null}
        </form> : <p>Loading settings…</p>}
        <button disabled={busy || !status.track} onClick={() => { if (status.track) void action(`/api/tracks/${status.track[0]}/${status.track[1]}/relearn`); }}>Relearn map</button>
      </section> : null}
    </>}
    <section id="notice"><h2>Notice</h2>
      <p>pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.</p>
      <p>This project is not endorsed by or affiliated with EA or its licensors.</p>
      <p>Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.</p>
    </section>
  </main>;
}
