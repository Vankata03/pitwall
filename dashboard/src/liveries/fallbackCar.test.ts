import { expect, test } from 'vitest';
import { NEUTRAL_COLOURS } from './choose';
import { FALLBACK_CAR_SIZE, fallbackCarMaskSvg, fallbackCarSvg } from './fallbackCar';

const colours = [
  { r: 201, g: 18, b: 31 },
  { r: 11, g: 42, b: 222 },
  { r: 240, g: 241, b: 242 },
  { r: 6, g: 7, b: 8 },
];

const attr = (element: string, name: string) =>
  Number(new RegExp(`\\b${name}="(-?[\\d.]+)"`).exec(element)![1]);

/** Every element of the markup carrying `marker`, as its opening tag. */
const tagged = (svg: string, marker: string) => svg.match(new RegExp(`<[^>]*${marker}[^>]*>`, 'g')) ?? [];

test('the car is a transparent 4:3 picture at 960 by 720', () => {
  expect(FALLBACK_CAR_SIZE).toEqual({ width: 960, height: 720 });
  const svg = fallbackCarSvg(colours);
  expect(svg).toMatch(/^<svg [^>]*xmlns="http:\/\/www\.w3\.org\/2000\/svg"/);
  expect(svg).toContain('viewBox="0 0 960 720"');
  expect(svg).toContain('width="960"');
  expect(svg).toContain('height="720"');
  // No backdrop: nothing covers the whole canvas.
  expect(svg).not.toMatch(/<rect[^>]*width="960"[^>]*height="720"/);
  expect(svg).not.toMatch(/<svg[^>]*(fill|style|background)=/);
});

test('the art is self-contained: no images, links, text or script', () => {
  for (const svg of [fallbackCarSvg(colours), fallbackCarMaskSvg()]) {
    expect(svg).not.toMatch(/<(image|text|script|foreignObject|use|a)\b/i);
    expect(svg).not.toMatch(/href=|url\((?!#)|data:|https?:\/\/(?!www\.w3\.org\/2000\/svg)/i);
  }
});

test('each of the four colour slots paints its own zone', () => {
  const svg = fallbackCarSvg(colours);
  for (const [slot, { r, g, b }] of colours.entries()) {
    const zone = tagged(svg, `data-zone="${slot}"`);
    expect(zone.length).toBeGreaterThan(0);
    for (const tag of zone) expect(tag).toContain(`fill="rgb(${r},${g},${b})"`);
  }
});

test('changing a colour changes only that zone', () => {
  const base = fallbackCarSvg(colours);
  const changed = fallbackCarSvg([colours[0], { r: 1, g: 2, b: 3 }, colours[2], colours[3]]);
  expect(changed).not.toBe(base);
  expect(tagged(changed, 'data-zone="0"')).toEqual(tagged(base, 'data-zone="0"'));
  expect(tagged(changed, 'data-zone="1"')).not.toEqual(tagged(base, 'data-zone="1"'));
  expect(tagged(changed, 'data-zone="2"')).toEqual(tagged(base, 'data-zone="2"'));
});

test('neutral colours draw a grey car', () => {
  const svg = fallbackCarSvg(NEUTRAL_COLOURS);
  for (const { r, g, b } of NEUTRAL_COLOURS) expect(svg).toContain(`rgb(${r},${g},${b})`);
});

test('a colour out of range or not a number is clamped, never written into the markup raw', () => {
  const svg = fallbackCarSvg([{ r: 999, g: -4, b: Number.NaN }, ...colours.slice(1)]);
  expect(tagged(svg, 'data-zone="0"')[0]).toContain('fill="rgb(255,0,0)"');
  expect(svg).not.toMatch(/NaN|undefined/);
});

test('the front axle sits below the rear axle', () => {
  const svg = fallbackCarSvg(colours);
  const centreY = (axle: string) =>
    tagged(svg, `data-axle="${axle}"`).map((tag) => attr(tag, 'y') + attr(tag, 'height') / 2);
  const [front, rear] = [centreY('front'), centreY('rear')];
  expect(front).toHaveLength(2);
  expect(rear).toHaveLength(2);
  expect(Math.min(...front)).toBeGreaterThan(Math.max(...rear));
});

test('the car is symmetric about the centre line, wheels included', () => {
  const svg = fallbackCarSvg(colours);
  for (const axle of ['front', 'rear']) {
    const [left, right] = tagged(svg, `data-axle="${axle}"`).map(
      (tag) => attr(tag, 'x') + attr(tag, 'width') / 2,
    );
    expect(left + right).toBe(FALLBACK_CAR_SIZE.width);
  }
});

test('the whole car stays inside the canvas', () => {
  const svg = fallbackCarSvg(colours);
  const numbers = [...svg.matchAll(/\bd="([^"]+)"/g)].flatMap((match) =>
    match[1].match(/-?\d+(\.\d+)?/g)!.map(Number),
  );
  expect(Math.min(...numbers)).toBeGreaterThanOrEqual(0);
  expect(Math.max(...numbers)).toBeLessThanOrEqual(960);
  for (const tag of tagged(svg, 'data-axle')) {
    expect(attr(tag, 'x')).toBeGreaterThanOrEqual(0);
    expect(attr(tag, 'x') + attr(tag, 'width')).toBeLessThanOrEqual(960);
    expect(attr(tag, 'y') + attr(tag, 'height')).toBeLessThanOrEqual(720);
  }
});

test('the mask marks the same four zones in four fixed colours, and nothing else', () => {
  const car = fallbackCarSvg(colours);
  const mask = fallbackCarMaskSvg();
  expect(mask).toContain('viewBox="0 0 960 720"');
  const maskFills = ['#ff0000', '#00ff00', '#0000ff', '#ffff00'];
  for (const [slot, fill] of maskFills.entries()) {
    const zone = tagged(mask, `data-zone="${slot}"`);
    expect(zone.map((tag) => tag.replace(/ d="[^"]*"/, ''))).toEqual(
      zone.map((tag) => tag.replace(/ d="[^"]*"/, '').replace(/fill="[^"]*"/, `fill="${fill}"`)),
    );
    expect(zone.length).toBe(tagged(car, `data-zone="${slot}"`).length);
    // Same shapes as the car's zones.
    expect(zone.map((tag) => / d="([^"]*)"/.exec(tag)![1])).toEqual(
      tagged(car, `data-zone="${slot}"`).map((tag) => / d="([^"]*)"/.exec(tag)![1]),
    );
  }
  expect(mask).not.toMatch(/data-axle|<rect/);
});
