import { createContext, useContext, useSyncExternalStore, type ReactNode } from 'react';
import { readoutView } from './readout';
import { bannersFor, type Runtime } from './runtime';
import type { SlowSnapshot } from './stores';
import type { ReadoutState } from './wire';

const RuntimeContext = createContext<Runtime | null>(null);

export function RuntimeProvider({ runtime, children }: { runtime: Runtime; children: ReactNode }) {
  return <RuntimeContext.Provider value={runtime}>{children}</RuntimeContext.Provider>;
}

export function useRuntime(): Runtime {
  const rt = useContext(RuntimeContext);
  if (!rt) throw new Error('useRuntime needs a RuntimeProvider');
  return rt;
}

/**
 * Slow path: State, History and connection status at 4 to 10 Hz. `select` must return a
 * stable value (a primitive, or a reference taken from the snapshot), never a fresh object.
 */
export function useSlow<T>(select: (snapshot: SlowSnapshot) => T): T {
  const { slow } = useRuntime().stores;
  const get = () => select(slow.getSnapshot());
  return useSyncExternalStore(slow.subscribe, get, get);
}

/** A readout for the slow path; frame tiles use `writeReadout` on a DOM node instead. */
export function Readout({ state, value }: { state: ReadoutState; value: string }) {
  const v = readoutView(state, value);
  return (
    <span className="pw-readout" data-state={v.state}>
      {v.text}
    </span>
  );
}

function BannerList({ runtime }: { runtime: Runtime }) {
  const snapshot = useSyncExternalStore(
    runtime.stores.slow.subscribe,
    runtime.stores.slow.getSnapshot,
    runtime.stores.slow.getSnapshot,
  );
  const prompt = useSyncExternalStore(runtime.guard.subscribe, runtime.guard.getSnapshot, runtime.guard.getSnapshot);
  const banners = bannersFor(snapshot, prompt);
  if (banners.length === 0) return null;
  return (
    <div className="pw-banners" role="status">
      {banners.map((b) => (
        <div className="pw-banner" data-banner={b.id} key={b.id}>
          <span>{b.text}</span>
          {b.hint ? <small>{b.hint}</small> : null}
          {b.action === 'reload' ? (
            <button type="button" onClick={() => runtime.guard.reloadNow()}>
              Reload
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}

/** Connection and game-state banners. Renders nothing outside a RuntimeProvider. */
export function Banners() {
  const rt = useContext(RuntimeContext);
  return rt ? <BannerList runtime={rt} /> : null;
}

function SlowDrawText({ runtime }: { runtime: Runtime }) {
  const { subscribe, getSnapshot } = runtime.loop.monitor;
  const slow = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  if (!slow) return null;
  return (
    <p className="pw-slow-draw" role="status">
      This screen is drawing slowly. Data and layout are unchanged.
    </p>
  );
}

/** Quiet notice for the display menu when draw rate stays under 24 fps for 5 s. */
export function SlowDrawNotice() {
  const rt = useContext(RuntimeContext);
  return rt ? <SlowDrawText runtime={rt} /> : null;
}
