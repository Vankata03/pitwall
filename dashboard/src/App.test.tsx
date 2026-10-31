import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test } from 'vitest';
import { App } from './App';

test('the dashboard identifies pitwall before telemetry is available', () => {
  expect(renderToStaticMarkup(<App />)).toContain('<h1>pitwall</h1>');
});
