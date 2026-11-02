// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './harness';
import { ReplayControls } from './replay-controls';
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
test('replay transport works from a connected dashboard and offers EOF actions', async () => {
  const fetcher = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal('fetch', fetcher);
  const view = mount();
  await act(async () => { await view.controller.start(); });
  act(() => view.sendState({ context: { replay: { position: 10, duration: 30, paused: false, speed: 1, banner: 'Replay', loadProgress: 1, backgroundLive: 'A live session is recording in the background' } } }));
  await waitFor(() => expect(screen.getByRole('button', { name: 'Pause replay' })).toBeTruthy());
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Pause replay' })); });
  await waitFor(() => expect(fetcher).toHaveBeenCalledWith('/api/replay/pause', expect.objectContaining({ body: '{"paused":true}' })));
  await act(async () => { fireEvent.change(screen.getByLabelText('Replay speed'), { target: { value: '2' } }); });
  await act(async () => { fireEvent.change(screen.getByLabelText('Replay position'), { target: { value: '20' } }); });
  await act(async () => { fireEvent.pointerUp(screen.getByLabelText('Replay position')); });
  await act(async () => { fireEvent.change(screen.getByLabelText('Replay lap'), { target: { value: '3' } }); });
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Jump to lap' })); });
  await waitFor(() => expect(fetcher).toHaveBeenCalledWith('/api/replay/lap', expect.objectContaining({ body: '{"car":0,"lap":3}' })));
  expect(fetcher).toHaveBeenCalledWith('/api/replay/speed', expect.objectContaining({ body: '{"speed":2}' }));
  expect(fetcher).toHaveBeenCalledWith('/api/replay/seek', expect.objectContaining({ body: '{"position":20}' }));
  expect(screen.getByText('A live session is recording in the background')).toBeTruthy();
  act(() => view.sendState({ context: { replay: { position: 30, duration: 30, paused: true, speed: 1, banner: 'Replay ended — restart or exit', loadProgress: 1, backgroundLive: null } } }));
  await waitFor(() => expect(screen.getByRole('button', { name: 'Restart replay' })).toBeTruthy());
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Restart replay' })); });
  await waitFor(() => expect(fetcher).toHaveBeenCalledWith('/api/replay/pause', expect.objectContaining({ body: '{"paused":false}' })));
  expect(screen.getByRole('link', { name: 'Exit replay on host' }).getAttribute('href')).toBe('/status');
});

const replay = { position: 10, duration: 30, paused: false, speed: 1, banner: 'Replay', loadProgress: 1, backgroundLive: null };

test('scrubbing follows the thumb locally and seeks once on release, without snapping back before acknowledgement', async () => {
  let finish!: (response: { ok: boolean }) => void;
  const fetcher = vi.fn(() => new Promise<{ ok: boolean }>(resolve => { finish = resolve; }));
  vi.stubGlobal('fetch', fetcher);
  const view = render(<ReplayControls replay={replay} car={0} epoch={1} />);
  const slider = screen.getByRole('slider') as HTMLInputElement;
  fireEvent.pointerDown(slider);
  for (let i = 11; i <= 20; i++) fireEvent.change(slider, { target: { value: String(i) } });
  expect(slider.value).toBe('20');
  expect(screen.getByText('20.0 / 30.0 s')).toBeTruthy();
  expect(slider.disabled || slider.closest('fieldset')!.disabled).toBe(false);
  expect(fetcher).not.toHaveBeenCalled();
  view.rerender(<ReplayControls replay={{ ...replay, position: 12 }} car={0} epoch={1} />);
  expect(slider.value).toBe('20');
  fireEvent.pointerUp(slider);
  fireEvent.blur(slider);
  expect(fetcher).toHaveBeenCalledTimes(1);
  expect(fetcher).toHaveBeenCalledWith('/api/replay/seek', expect.objectContaining({ body: '{"position":20}' }));
  expect(slider.value).toBe('20');
  await act(async () => finish({ ok: true }));
  expect(slider.value).toBe('20');
  view.rerender(<ReplayControls replay={{ ...replay, position: 20 }} car={0} epoch={2} />);
  view.rerender(<ReplayControls replay={{ ...replay, position: 21 }} car={0} epoch={2} />);
  expect((screen.getByRole('slider') as HTMLInputElement).value).toBe('21');
});

test('keyboard seeks commit on key release and cancelled pointer gestures do not seek', async () => {
  const fetcher = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal('fetch', fetcher);
  render(<ReplayControls replay={replay} car={0} epoch={1} />);
  const slider = screen.getByRole('slider') as HTMLInputElement;
  fireEvent.change(slider, { target: { value: '14' } });
  fireEvent.pointerCancel(slider);
  expect(slider.value).toBe('10');
  expect(fetcher).not.toHaveBeenCalled();
  fireEvent.change(slider, { target: { value: '15' } });
  fireEvent.change(slider, { target: { value: '16' } });
  expect(fetcher).not.toHaveBeenCalled();
  await act(async () => { fireEvent.keyUp(slider, { key: 'ArrowRight' }); });
  expect(fetcher).toHaveBeenCalledTimes(1);
  expect(fetcher).toHaveBeenCalledWith('/api/replay/seek', expect.objectContaining({ body: '{"position":16}' }));
});

test('failed seeks show the error and restore the published position', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }));
  render(<ReplayControls replay={replay} car={0} epoch={1} />);
  const slider = screen.getByRole('slider') as HTMLInputElement;
  fireEvent.change(slider, { target: { value: '20' } });
  await act(async () => { fireEvent.pointerUp(slider); });
  expect(slider.value).toBe('10');
  expect(screen.getByRole('alert').textContent).toContain('Replay seek failed: 500');
});
test('loading replay shows progress without enabling transport', async () => {
  const view = mount();
  await act(async () => { await view.controller.start(); });
  act(() => view.sendState({ context: { replay: { position: 0, duration: 0, paused: true, speed: 1, banner: 'Loading replay', loadProgress: 0.4, backgroundLive: null } } }));
  await waitFor(() => expect(screen.getByRole('progressbar').getAttribute('value')).toBe('0.4'));
  expect(screen.getByRole('button', { name: 'Resume replay' }).closest('fieldset')!.disabled).toBe(true);
});
