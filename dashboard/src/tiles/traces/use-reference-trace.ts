import { useEffect, useRef, useSyncExternalStore } from 'react';
import { useRuntime, useSlow } from '../../runtime/react';
import { lapTracesFor } from './lap-traces';

/**
 * Keep the reference lap trace of a car loaded: request it, and refetch when a lap completes or
 * the session changes. `traces.get(index, referenceLap)` returns it once it has arrived; the
 * returned `version` changes when it does, so pass it as a redraw dependency.
 */
export function useReferenceTrace(index: number | null, reference: 'best' | 'previous') {
  const runtime = useRuntime();
  const slow = useSlow((s) => s);
  const traces = lapTracesFor(runtime);
  const version = useSyncExternalStore(traces.subscribe, () => traces.version);
  const info = index === null ? null : slow.state?.cars?.[index];
  const referenceLap: number | undefined = info?.[reference]?.lap_number;

  useEffect(() => {
    if (index !== null && referenceLap !== undefined) traces.request(index, referenceLap);
  }, [traces, index, referenceLap, slow.epoch, slow.connection]);

  const history = useRef(slow.history);
  useEffect(() => {
    const changed = slow.history.filter((entry) => entry.kind === 'lap' && !history.current.includes(entry));
    history.current = slow.history;
    if (!changed.length || index === null) return;
    traces.invalidate();
    for (const entry of changed) {
      if (entry.kind === 'lap' && entry.car === index) {
        traces.invalidate(index, entry.body.lap_number);
        traces.request(index, entry.body.lap_number);
      }
    }
    for (const n of [info?.previous?.lap_number, info?.best?.lap_number]) {
      if (n !== undefined) traces.request(index, n);
    }
  }, [slow.history, info, index, traces]);

  return { slow, traces, version, referenceLap };
}
