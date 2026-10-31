/** Test-only: a whole dashboard wired to a fake collector, in jsdom. */
import { render } from '@testing-library/react';
import { vi } from 'vitest';
import { createDevice, memoryStorage } from '../device/device';
import { createLayoutController } from '../layout/controller';
import { fakeServer, layoutOf } from '../layout/fake-server';
import { hello, snapshot } from '../runtime/fixtures';
import { RuntimeProvider } from '../runtime/react';
import { createRuntime } from '../runtime/runtime';
import { createRegistry, registry as shipped, type TileRegistry } from '../tiles/registry';
import { App } from '../App';

export function mockViewport(width: number): void {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => width - 16 });
}

export function mount(
  options: {
    registry?: TileRegistry;
    extra?: ReturnType<typeof layoutOf>[];
    stored?: Record<string, string>;
    viewport?: number;
    fixed?: string | null;
  } = {},
) {
  mockViewport(options.viewport ?? 1280);
  const server = fakeServer(options.extra);
  const device = createDevice(memoryStorage(options.stored));
  const runtime = createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({ binaryType: '', onopen: null, onmessage: null, onclose: null, onerror: null, close() {} }),
    requestFrame: () => 0,
    cancelFrame: () => {},
    reload: () => {},
    storage: { getItem: () => null, setItem: () => {} },
  });
  const setEditing = vi.spyOn(runtime, 'setEditing');
  const controller = createLayoutController({
    api: server,
    device,
    fixedLayout: options.fixed ?? null,
    onEditingChange: (e) => runtime.setEditing(e),
  });
  const registry = options.registry ?? shipped;
  const view = render(
    <RuntimeProvider runtime={runtime}>
      <App controller={controller} device={device} registry={registry} />
    </RuntimeProvider>,
  );
  /** Push a State snapshot to this screen, as the collector's stream would. */
  const sendState = (state: Record<string, unknown>, epoch = 1) => {
    runtime.stores.setConnection('open');
    runtime.stores.ingest(hello(epoch));
    runtime.stores.ingest(snapshot(epoch, { lifecycle: 'live', ...state }));
  };
  return { ...view, server, device, runtime, controller, setEditing, sendState, registry };
}

export { createRegistry };
