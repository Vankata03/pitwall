import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, test, vi } from 'vitest';
import { hello, snapshot } from './fixtures';
import { Banners, Readout, RuntimeProvider, SlowDrawNotice, useSlow } from './react';
import { createRuntime } from './runtime';
import { ReadoutState } from './wire';

function runtime() {
  return createRuntime({
    url: 'ws://h/ws',
    ownBuildId: 'b1',
    createSocket: () => ({
      binaryType: '',
      onopen: null,
      onmessage: null,
      onclose: null,
      onerror: null,
      close() {},
    }),
    requestFrame: () => 0,
    cancelFrame: () => {},
    reload: () => {},
    storage: { getItem: () => null, setItem: () => {} },
  });
}

describe('Readout component', () => {
  test.each([
    [ReadoutState.Live, '312', '<span class="pw-readout" data-state="live">312</span>'],
    [ReadoutState.Stale, '312', '<span class="pw-readout" data-state="stale">312</span>'],
    [ReadoutState.Restricted, '0', '<span class="pw-readout" data-state="restricted">Restricted</span>'],
    [ReadoutState.Absent, '0', '<span class="pw-readout" data-state="absent">—</span>'],
  ])('state %i', (state, value, html) => {
    expect(renderToStaticMarkup(<Readout state={state} value={value} />)).toBe(html);
  });
});

describe('Banners', () => {
  test('render nothing without a runtime', () => {
    expect(renderToStaticMarkup(<Banners />)).toBe('');
  });

  test('show the connecting banner before the first message', () => {
    const rt = runtime();
    const html = renderToStaticMarkup(
      <RuntimeProvider runtime={rt}>
        <Banners />
      </RuntimeProvider>,
    );
    expect(html).toContain('Connecting to the collector');
    expect(html).toContain('role="status"');
  });

  test('show the waiting banner with the setup hint once connected', () => {
    vi.useFakeTimers();
    const rt = runtime();
    rt.stores.setConnection('open');
    rt.stores.ingest(hello(1));
    rt.stores.ingest(snapshot(1, { lifecycle: 'waiting' }));
    vi.advanceTimersByTime(200);
    vi.useRealTimers();
    const html = renderToStaticMarkup(
      <RuntimeProvider runtime={rt}>
        <Banners />
      </RuntimeProvider>,
    );
    expect(html).toContain('Waiting for the game');
    expect(html).toContain('20777');
  });
});

describe('SlowDrawNotice', () => {
  test('is empty at normal frame rate and quiet when shown', () => {
    const rt = runtime();
    const wrap = (c: React.ReactNode) => renderToStaticMarkup(<RuntimeProvider runtime={rt}>{c}</RuntimeProvider>);
    expect(wrap(<SlowDrawNotice />)).toBe('');
    rt.loop.monitor.slow = true;
    const html = wrap(<SlowDrawNotice />);
    expect(html).toContain('drawing slowly');
  });
});

describe('useSlow', () => {
  test('reads a selected slice of the slow snapshot', () => {
    const rt = runtime();
    function Probe() {
      const c = useSlow((s) => s.connection);
      return <p>{c}</p>;
    }
    expect(
      renderToStaticMarkup(
        <RuntimeProvider runtime={rt}>
          <Probe />
        </RuntimeProvider>,
      ),
    ).toBe('<p>connecting</p>');
  });
});
