import { expect, test } from 'vitest';
import { chooseLivery, NEUTRAL_COLOURS, type LiveryCar } from './choose';

const red = { r: 200, g: 20, b: 30 };
const blue = { r: 10, g: 40, b: 220 };
const white = { r: 250, g: 250, b: 250 };
const black = { r: 5, g: 5, b: 5 };

const car = (overrides: Partial<LiveryCar> = {}): LiveryCar => ({
  teamId: 3,
  myTeam: 0,
  numColours: 2,
  colours: [red, blue, white, black],
  ...overrides,
});

test('My Team uses my-team.png, whatever its team id', () => {
  const choice = chooseLivery(car({ teamId: 255, myTeam: 1 }), ['3.png', 'my-team.png']);
  expect(choice).toMatchObject({ source: 'pack', file: 'my-team.png', url: '/liveries/my-team.png' });
});

test('a team car uses its own <teamId>.png', () => {
  const choice = chooseLivery(car({ teamId: 481 }), ['3.png', '481.png', 'my-team.png']);
  expect(choice).toMatchObject({ source: 'pack', file: '481.png', url: '/liveries/481.png' });
});

test('a car with no matching file gets the fallback', () => {
  expect(chooseLivery(car({ teamId: 7 }), ['3.png', 'readme.txt']).source).toBe('fallback');
  expect(chooseLivery(car({ teamId: 7 }), []).source).toBe('fallback');
});

test('My Team without my-team.png falls through to its <teamId>.png, then the fallback', () => {
  expect(chooseLivery(car({ teamId: 3, myTeam: 1 }), ['3.png'])).toMatchObject({ source: 'pack', file: '3.png' });
  expect(chooseLivery(car({ teamId: 3, myTeam: 1 }), ['4.png']).source).toBe('fallback');
});

test('files that match nothing are ignored, including near misses', () => {
  const files = ['03.png', '3.PNG', '3.png.bak', 'team-3.png', '/liveries/3.png', '../3.png'];
  expect(chooseLivery(car({ teamId: 3 }), files).source).toBe('fallback');
});

test('game colours come first, in game order', () => {
  const choice = chooseLivery(car({ numColours: 3 }), []);
  expect(choice.colours.slice(0, 3)).toEqual([red, blue, white]);
});

test('colour slots the game did not send are neutral', () => {
  const choice = chooseLivery(car({ numColours: 2 }), []);
  expect(choice.colours).toEqual([red, blue, NEUTRAL_COLOURS[2], NEUTRAL_COLOURS[3]]);
});

test('no game colours means all neutral', () => {
  const choice = chooseLivery(car({ numColours: 0 }), []);
  expect(choice.colours).toEqual(NEUTRAL_COLOURS);
});

test('a numColours above the four slots is capped', () => {
  const choice = chooseLivery(car({ numColours: 9 }), []);
  expect(choice.colours).toEqual([red, blue, white, black]);
});

test('a missing car (no participant data yet) gets the neutral fallback', () => {
  expect(chooseLivery(undefined, ['3.png'])).toEqual({ source: 'fallback', colours: NEUTRAL_COLOURS });
});
