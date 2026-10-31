/**
 * The fallback car: one original, neutral, front-facing car, drawn here as code. It is
 * not traced from, or derived from, any real car or pack image, and carries no logo,
 * number or text. One body serves both regulations.
 *
 * The picture is 960 by 720 (4:3) on a transparent canvas, like a pack image, viewed from
 * in front and slightly above, so the front axle sits lower in the picture than the rear
 * axle. The car is symmetric about the centre line; the driver's left is the image's
 * right.
 *
 * The recolour mask is the four colour *zones*. [`fallbackCarSvg`] paints them with the
 * livery colours; [`fallbackCarMaskSvg`] draws the same shapes in four flat mask colours
 * for a caller that recolours by canvas. Everything else (tyres, suspension, halo,
 * helmet, shading) is fixed and never takes a livery colour.
 *
 * Use the result as an `<img>` source (a `data:` URI) or inline; either way it holds no
 * external reference.
 */
import type { Rgb } from './choose';

export const FALLBACK_CAR_SIZE = { width: 960, height: 720 } as const;

const CENTRE = FALLBACK_CAR_SIZE.width / 2;

type Point = readonly [number, number];

/** Reflect a left-hand outline across the centre line, keeping its winding. */
const mirror = (points: readonly Point[]): Point[] =>
  points.map(([x, y]): Point => [2 * CENTRE - x, y]).reverse();

const path = (points: readonly Point[]) => `M${points.map(([x, y]) => `${x} ${y}`).join('L')}Z`;

/** A left outline plus its mirror image. */
const pair = (left: readonly Point[]) => [path(left), path(mirror(left))];

/** The four recolour zones, as lists of closed outlines. Slot order is the game's. */
const ZONES: readonly (readonly string[])[] = [
  // 0 body: airbox, sidepods, tub, nose.
  [
    path([[438, 196], [522, 196], [546, 336], [414, 336]]),
    ...pair([[322, 336], [412, 306], [420, 474], [340, 486], [302, 424]]),
    path([[408, 300], [552, 300], [574, 468], [524, 566], [436, 566], [386, 468]]),
    path([[448, 520], [512, 520], [502, 596], [480, 616], [458, 596]]),
  ],
  // 1 secondary: nose stripe, engine cover stripe, sidepod inlet bars.
  [
    path([[473, 520], [487, 520], [485, 604], [475, 604]]),
    path([[470, 200], [490, 200], [494, 330], [466, 330]]),
    ...pair([[340, 344], [412, 322], [412, 346], [338, 372]]),
  ],
  // 2 wings: rear wing main plane and flap, front wing main plane and flap.
  [
    path([[352, 124], [608, 124], [616, 168], [344, 168]]),
    path([[364, 168], [596, 168], [600, 188], [360, 188]]),
    path([[176, 644], [784, 644], [796, 680], [164, 680]]),
    path([[214, 614], [746, 614], [758, 644], [202, 644]]),
  ],
  // 3 trim: wing endplates and the sidepod floor edge.
  [
    ...pair([[330, 108], [350, 108], [350, 200], [330, 200]]),
    ...pair([[152, 596], [176, 596], [176, 690], [152, 690]]),
    ...pair([[304, 428], [340, 490], [424, 478], [424, 492], [336, 506], [296, 432]]),
  ],
];

/** Flat mask colours for the four zones, in slot order. */
const MASK_FILLS = ['#ff0000', '#00ff00', '#0000ff', '#ffff00'] as const;

const channel = (value: number) => (Number.isFinite(value) ? Math.round(Math.min(255, Math.max(0, value))) : 0);
const css = ({ r, g, b }: Rgb) => `rgb(${channel(r)},${channel(g)},${channel(b)})`;

const wheel = (axle: 'front' | 'rear', x: number, y: number, width: number, height: number) =>
  `<rect data-axle="${axle}" x="${x}" y="${y}" width="${width}" height="${height}" rx="${Math.round(width / 6)}" fill="#14171a" stroke="#3a4348" stroke-width="2"/>` +
  // Tread highlight and sidewall band, so the tyre reads as a cylinder.
  `<rect x="${x + 6}" y="${y + 8}" width="${width - 12}" height="${height * 0.12}" rx="4" fill="#2a3035"/>` +
  `<rect x="${x + width * 0.18}" y="${y + height * 0.3}" width="${width * 0.64}" height="${height * 0.4}" rx="8" fill="#232a2e"/>`;

const wheels = () => {
  const wheelPair = (axle: 'front' | 'rear', offset: number, y: number, width: number, height: number) =>
    wheel(axle, CENTRE - offset - width / 2, y, width, height) +
    wheel(axle, CENTRE + offset - width / 2, y, width, height);
  return {
    rear: wheelPair('rear', 218, 290, 120, 184),
    front: wheelPair('front', 312, 452, 140, 204),
  };
};

const line = (x1: number, y1: number, x2: number, y2: number, width: number) =>
  `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="#262c30" stroke-width="${width}" stroke-linecap="round"/>`;

/** The fixed parts drawn under the zones. */
const under = () => {
  const { rear, front } = wheels();
  const arms = [-1, 1]
    .map((side) => {
      const x = (distance: number) => CENTRE + side * distance;
      return (
        line(x(70), 360, x(158), 372, 8) +
        line(x(70), 410, x(158), 400, 8) +
        line(x(50), 530, x(250), 540, 9) +
        line(x(40), 566, x(250), 576, 9)
      );
    })
    .join('');
  return (
    `<ellipse cx="${CENTRE}" cy="652" rx="340" ry="44" fill="url(#shadow)"/>` +
    rear +
    arms +
    front +
    // Pylons carrying the front wing.
    `<rect x="446" y="590" width="14" height="40" fill="#262c30"/>` +
    `<rect x="500" y="590" width="14" height="40" fill="#262c30"/>`
  );
};

/** Fixed parts drawn over the zones: cockpit opening, helmet, halo, airbox intake. */
const over = () =>
  `<ellipse cx="${CENTRE}" cy="228" rx="26" ry="14" fill="#101315"/>` +
  `<ellipse cx="${CENTRE}" cy="352" rx="46" ry="26" fill="#101315"/>` +
  `<circle cx="${CENTRE}" cy="344" r="25" fill="#d6dbde"/>` +
  `<rect x="454" y="334" width="52" height="16" rx="7" fill="#1d2326"/>` +
  `<path d="M424 378Q426 318 ${CENTRE} 300Q534 318 536 378" fill="none" stroke="#1f2528" stroke-width="12" stroke-linecap="round"/>` +
  `<line x1="${CENTRE}" y1="300" x2="${CENTRE}" y2="404" stroke="#1f2528" stroke-width="11" stroke-linecap="round"/>`;

const SHADE =
  '<defs>' +
  // Dark toward the edges, so flat zone colours read as rounded.
  '<linearGradient id="shade" x1="0" x2="1" y1="0" y2="0">' +
  '<stop offset="0" stop-color="#000" stop-opacity=".34"/>' +
  '<stop offset=".5" stop-color="#fff" stop-opacity=".1"/>' +
  '<stop offset="1" stop-color="#000" stop-opacity=".34"/>' +
  '</linearGradient>' +
  '<radialGradient id="shadow"><stop offset="0" stop-color="#000" stop-opacity=".45"/>' +
  '<stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient>' +
  '</defs>';

const open = () =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${FALLBACK_CAR_SIZE.width}" height="${FALLBACK_CAR_SIZE.height}" viewBox="0 0 ${FALLBACK_CAR_SIZE.width} ${FALLBACK_CAR_SIZE.height}">`;

/** The car painted with up to four livery colours, one per zone, in slot order. */
export function fallbackCarSvg(colours: readonly Rgb[]): string {
  const zones = ZONES.map((outlines, slot) => {
    const fill = css(colours[slot] ?? { r: 0, g: 0, b: 0 });
    return outlines
      .map((d) => `<path data-zone="${slot}" d="${d}" fill="${fill}"/><path d="${d}" fill="url(#shade)"/>`)
      .join('');
  }).join('');
  return `${open()}${SHADE}${under()}${zones}${over()}</svg>`;
}

/** The recolour mask: the same zones in flat red, green, blue and yellow, no fixed parts. */
export function fallbackCarMaskSvg(): string {
  const zones = ZONES.map((outlines, slot) =>
    outlines.map((d) => `<path data-zone="${slot}" d="${d}" fill="${MASK_FILLS[slot]}"/>`).join(''),
  ).join('');
  return `${open()}${zones}</svg>`;
}
