import { expect, test } from 'vitest';
import { streamFromUrl, streamUnits } from './stream';

test('Stream URL selects an arrangement and overrides units independently without storage', () => {
  const options = streamFromUrl('?stream=1&layout=On+air&class=phone&speed=mph&temp=f&pressure=bar&focus=2');
  expect(options).toMatchObject({ enabled: true, layout: 'On air', screenClass: 'phone' });
  expect(streamUnits(options, { speed: 'kmh', temperature: 'c', pressure: 'psi' })).toEqual({ speed: 'mph', temperature: 'f', pressure: 'bar' });
  const invalid = streamFromUrl('?stream=1&class=huge&speed=fast&temp=c');
  expect(invalid.screenClass).toBeUndefined();
  expect(streamUnits(invalid, { speed: 'mph', temperature: 'f', pressure: 'bar' })).toEqual({ speed: 'mph', temperature: 'c', pressure: 'bar' });
  expect(streamFromUrl('?stream=0').enabled).toBe(false);
});
