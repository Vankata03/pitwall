// @vitest-environment jsdom
import { act, cleanup, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { mount } from './app/harness';

afterEach(cleanup);

test('the app is the dashboard: the pitwall wordmark, then the chrome and the layout', async () => {
  const view = mount();
  await act(async () => {
    await view.controller.start();
  });
  expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('pitwall');
  expect(screen.getByRole('button', { name: 'Edit' })).toBeTruthy();
});
