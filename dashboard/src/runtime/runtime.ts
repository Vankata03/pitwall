import { deriveBanners, type Banner } from './banners';
import { BuildGuard } from './build-guard';
import { CollectorClient, type SocketLike } from './client';
import { RenderLoop } from './loop';
import { createStores, type SlowSnapshot, type Stores } from './stores';
import { lifecycleOf } from './wire';

export interface RuntimeOptions {
  /** WebSocket URL; defaults to `/ws` on the page's host. */
  url?: string;
  /** Build ID baked into this page, compared with the collector's hello. */
  ownBuildId: string;
  createSocket?: (url: string) => SocketLike;
  requestFrame?: (cb: (ts: number) => void) => number;
  cancelFrame?: (id: number) => void;
  reload?: () => void;
  storage?: Pick<Storage, 'getItem' | 'setItem'>;
  silenceMs?: number;
}

/** Everything a screen needs from the collector: stores, the connection, the frame loop. */
export interface Runtime {
  readonly stores: Stores;
  readonly loop: RenderLoop;
  readonly guard: BuildGuard;
  start(): void;
  stop(): void;
  /** Tell the build guard about edit mode, so a build mismatch prompts instead of reloading. */
  setEditing(editing: boolean): void;
}

function defaultUrl(): string {
  const { protocol, host } = globalThis.location;
  return `${protocol === 'https:' ? 'wss:' : 'ws:'}//${host}/ws`;
}

export function createRuntime(options: RuntimeOptions): Runtime {
  const guard = new BuildGuard({
    ownBuildId: options.ownBuildId,
    reload: options.reload ?? (() => globalThis.location.reload()),
    storage: options.storage ?? globalThis.sessionStorage,
  });
  const stores = createStores({ onHello: (h) => guard.onHello(h.buildId) });
  const loop = new RenderLoop({
    stores,
    requestFrame: options.requestFrame,
    cancelFrame: options.cancelFrame,
  });
  const client = new CollectorClient({
    url: options.url ?? defaultUrl(),
    createSocket: options.createSocket,
    silenceMs: options.silenceMs,
    onMessage: (d) => stores.ingest(d),
    onStatus: (s) => stores.setConnection(s),
  });
  const onVisibility = () => loop.visibilityChanged();

  return {
    stores,
    loop,
    guard,
    start() {
      client.start();
      loop.start();
      globalThis.document?.addEventListener('visibilitychange', onVisibility);
    },
    stop() {
      client.stop();
      loop.stop();
      globalThis.document?.removeEventListener('visibilitychange', onVisibility);
    },
    setEditing: (editing) => guard.setEditing(editing),
  };
}

/** Banners for the current slow snapshot and reload prompt. */
export function bannersFor(snapshot: SlowSnapshot, reloadPrompt: boolean): Banner[] {
  return deriveBanners({
    connection: snapshot.connection,
    lifecycle: lifecycleOf(snapshot.state),
    reloadPrompt,
  });
}
