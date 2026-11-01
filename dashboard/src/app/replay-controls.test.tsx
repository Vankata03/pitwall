// @vitest-environment jsdom
import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './harness';
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
test('loading replay shows progress without enabling transport', async () => {
  const view = mount();
  await act(async () => { await view.controller.start(); });
  act(() => view.sendState({ context: { replay: { position: 0, duration: 0, paused: true, speed: 1, banner: 'Loading replay', loadProgress: 0.4, backgroundLive: null } } }));
  await waitFor(() => expect(screen.getByRole('progressbar').getAttribute('value')).toBe('0.4'));
  expect(screen.getByRole('button', { name: 'Resume replay' }).closest('fieldset')!.disabled).toBe(true);
});
