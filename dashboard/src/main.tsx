import { streamFromUrl } from './app/stream';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { createDevice, layoutNameFromUrl, memoryStorage, type StorageLike } from './device/device';
import { createLayoutController } from './layout/controller';
import { createLayoutsApi } from './layout/api';
import { RuntimeProvider } from './runtime/react';
import { createRuntime } from './runtime/runtime';
import { registry } from './tiles/registry';
import './styles/tokens.css';
import './styles/base.css';
import './runtime/runtime.css';
import './styles/app.css';
import 'react-grid-layout/css/styles.css';

const runtime = createRuntime({ ownBuildId: __PITWALL_BUILD_ID__ });

function browserStorage(): StorageLike {
  try {
    return globalThis.localStorage ?? memoryStorage();
  } catch {
    return memoryStorage(); // storage blocked: choices last until the page closes
  }
}

const stream = streamFromUrl(globalThis.location.search);
const device = createDevice(browserStorage());
let missingStreamLayout: string | null = null;
// Plain LAN HTTP is not a secure context; randomUUID is unavailable there.
const streamClient = Array.from(globalThis.crypto.getRandomValues(new Uint32Array(4)), (value) => value.toString(16)).join('-');
const reportMissing = () => fetch('/api/status/missing-stream', {
  method: 'POST', headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ client: streamClient, name: missingStreamLayout }),
}).then((response) => { if (!response.ok) console.error('Stream layout report rejected', response.status); }).catch(console.error);
if (stream.enabled) setInterval(() => { void reportMissing(); }, 30000);
const controller = createLayoutController({
  api: createLayoutsApi(),
  device,
  fixedLayout: stream.enabled ? stream.layout : layoutNameFromUrl(globalThis.location.search),
  stream: stream.enabled,
  onMissingStreamLayout: (name) => { missingStreamLayout = name; void reportMissing(); },
  onEditingChange: (editing) => runtime.setEditing(editing),
});

runtime.start();
if (globalThis.location.pathname !== '/status') void controller.start();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RuntimeProvider runtime={runtime}>
      <App controller={controller} device={device} registry={registry} stream={stream} />
    </RuntimeProvider>
  </StrictMode>,
);
