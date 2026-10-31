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
const controller = createLayoutController({
  api: createLayoutsApi(),
  device,
  fixedLayout: stream.enabled ? stream.layout : layoutNameFromUrl(globalThis.location.search),
  stream: stream.enabled,
  onEditingChange: (editing) => runtime.setEditing(editing),
});

runtime.start();
void controller.start();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RuntimeProvider runtime={runtime}>
      <App controller={controller} device={device} registry={registry} stream={stream} />
    </RuntimeProvider>
  </StrictMode>,
);
