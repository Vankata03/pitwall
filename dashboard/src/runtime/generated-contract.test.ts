import { expect, test, vi } from 'vitest';
import type { TextMessage } from '../generated/wire/TextMessage';
import { createStores } from './stores';
import { bannersFor } from './runtime';

// Assumed collector messages checked against the generated Rust contracts.
test('generated snapshot and State retain History bodies and drive lifecycle banners', () => {
  vi.useFakeTimers();
  try {
    const stores = createStores();
    const state: Extract<TextMessage, { type: 'snapshot' }>['state'] = {
      lifecycle: 'Waiting', session: { state: 'absent' }, playerCar: null,
      pitWindow: { state: 'absent' }, cars: Array(24).fill(null),
      context: { revisions: { layouts: 2, tracks: 3 }, replay: null,
        collector: { packetRateHz: 0, dropped: 0, parseErrors: 0, uidZero: 0,
          closedUid: 0, forwardingErrors: 0, lastFormat: null } },
    };
    const history: Extract<TextMessage, { type: 'snapshot' }>['history'] = [{
      id: 'lap:0:1', kind: 'lap', car: 0,
      body: { lap_number: 1, session_time: 80, lap_time_ms: 80000,
        sectors_ms: [26000, 27000, 27000], valid: true,
        sector_valid: [true, true, true], authoritative: true },
    }];
    const messages: TextMessage[] = [
      { type: 'hello', epoch: 7, source: 'live', buildId: 'contract-test' },
      { type: 'snapshot', epoch: 7, source: 'live', state, history },
      { type: 'state', epoch: 7, ...state, lifecycle: 'Lost' },
    ];
    stores.setConnection('open');
    for (const message of messages) stores.ingest(JSON.stringify(message));
    vi.advanceTimersByTime(200);
    expect(stores.slow.getSnapshot().history).toEqual(history);
    expect(stores.slow.getSnapshot().state?.context.revisions.tracks).toBe(3);
    expect(bannersFor(stores.slow.getSnapshot(), false)[0]?.text).toBe('No data: game paused or closed');
  } finally { vi.useRealTimers(); }
});
