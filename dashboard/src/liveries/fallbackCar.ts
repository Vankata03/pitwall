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

/** Reflect an absolute path across the centre line. Only M, L, C, Q and Z, with pairs of numbers. */
const mirrorPath = (d: string) => {
  let isX = true;
  return d.replace(/[A-Z]|-?\d+(\.\d+)?/g, (token) => {
    if (/[A-Z]/.test(token)) {
      isX = true;
      return token;
    }
    const value = isX ? 2 * CENTRE - Number(token) : Number(token);
    isX = !isX;
    return String(value);
  });
};

/** A left outline plus its mirror image. */
const pair = (left: string) => [left, mirrorPath(left)];

/** The four recolour zones, as lists of closed outlines. Slot order is the game's. */
const ZONES: readonly (readonly string[])[] = [
  // 0 body: engine cover, sidepods, tub and nose.
  [
    'M452 112C438 150 426 212 412 302L548 302C534 212 522 150 508 112Z',
    ...pair('M426 190C398 220 350 270 322 338C304 382 300 420 312 450C342 462 384 476 420 492L436 492L428 300Z'),
    'M412 288C398 338 388 402 396 468C404 524 430 566 438 604C442 628 454 646 468 650L492 650C506 646 518 628 522 604C530 566 556 524 564 468C572 402 562 338 548 288Z',
  ],
  // 1 secondary: nose stripe, engine cover spine, sidepod flash.
  [
    'M464 424L496 424L490 646L470 646Z',
    'M470 116L490 116L496 292L464 292Z',
    ...pair('M332 344C352 298 388 252 424 218L426 256C398 286 372 330 358 376Z'),
  ],
  // 2 wings: rear main plane and flap, front main plane and flaps.
  [
    'M300 42C380 28 580 28 660 42L668 94C580 82 380 82 292 94Z',
    'M308 96C390 86 570 86 652 96L648 118C570 108 390 108 312 118Z',
    'M72 640C200 598 340 618 440 648L520 648C620 618 760 598 888 640L898 684C760 650 620 676 520 692L440 692C340 676 200 650 62 684Z',
    ...pair('M96 610C204 578 322 590 430 624L430 642C322 612 204 602 90 634Z'),
  ],
  // 3 trim: wing endplates and the sidepod floor edge.
  [
    ...pair('M278 24L306 28L300 124L270 120Z'),
    ...pair('M50 594L96 588L90 696L42 702Z'),
    ...pair('M296 446C306 492 352 526 412 534L412 548C344 540 290 502 282 450Z'),
  ],
];

/** Flat mask colours for the four zones, in slot order. */
const MASK_FILLS = ['#ff0000', '#00ff00', '#0000ff', '#ffff00'] as const;

const channel = (value: number) => (Number.isFinite(value) ? Math.round(Math.min(255, Math.max(0, value))) : 0);
const css = ({ r, g, b }: Rgb) => `rgb(${channel(r)},${channel(g)},${channel(b)})`;

const wheel = (axle: 'front' | 'rear', x: number, y: number, width: number, height: number) =>
  `<rect data-axle="${axle}" x="${x}" y="${y}" width="${width}" height="${height}" rx="${Math.round(width / 5)}" fill="url(#tyre)"/>` +
  // Tread grooves and a soft top light, so the tyre reads as a cylinder.
  `<rect x="${x + width * 0.3}" y="${y + 6}" width="3" height="${height - 12}" fill="#000" opacity=".35"/>` +
  `<rect x="${x + width * 0.68}" y="${y + 6}" width="3" height="${height - 12}" fill="#000" opacity=".35"/>` +
  `<rect x="${x + 8}" y="${y + 8}" width="${width - 16}" height="${height * 0.2}" rx="10" fill="#fff" opacity=".05"/>`;

const wheels = () => {
  const wheelPair = (axle: 'front' | 'rear', offset: number, y: number, width: number, height: number) =>
    wheel(axle, CENTRE - offset - width / 2, y, width, height) +
    wheel(axle, CENTRE + offset - width / 2, y, width, height);
  return {
    rear: wheelPair('rear', 262, 138, 112, 194),
    front: wheelPair('front', 362, 366, 132, 214),
  };
};

const line = (x1: number, y1: number, x2: number, y2: number, width: number) =>
  `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="#2a3136" stroke-width="${width}" stroke-linecap="round"/>`;

/** The fixed parts drawn under the zones. */
const under = () => {
  const { rear, front } = wheels();
  const arms = [-1, 1]
    .map((side) => {
      const x = (distance: number) => CENTRE + side * distance;
      return (
        line(x(120), 200, x(214), 210, 7) +
        line(x(120), 262, x(214), 252, 7) +
        line(x(60), 438, x(298), 448, 9) +
        line(x(60), 510, x(298), 492, 9) +
        line(x(70), 462, x(298), 470, 5)
      );
    })
    .join('');
  return (
    `<ellipse cx="${CENTRE}" cy="420" rx="430" ry="290" fill="url(#shadow)"/>` +
    // The floor, a dark plate under the bodywork.
    `<path d="M330 250C280 340 268 470 340 556L620 556C692 470 680 340 630 250Z" fill="#101315"/>` +
    rear +
    front +
    arms +
    // Rear wing support and front wing pylons.
    `<path d="M462 96L498 96L506 160L454 160Z" fill="#1c2226"/>` +
    `<rect x="444" y="596" width="12" height="60" fill="#1c2226"/>` +
    `<rect x="504" y="596" width="12" height="60" fill="#1c2226"/>`
  );
};

/** Fixed parts drawn over the zones: intakes, cockpit, helmet, halo, mirrors and highlights. */
const over = () => {
  const mirrors = [-1, 1]
    .map((side) => {
      const x = (distance: number) => CENTRE + side * distance;
      return line(x(62), 300, x(108), 288, 5) + `<ellipse cx="${x(116)}" cy="286" rx="16" ry="8" fill="#1a2024"/>`;
    })
    .join('');
  const inlets = pair('M314 428C344 432 380 444 412 460L412 486C380 470 344 456 316 450Z')
    .map((d) => `<path d="${d}" fill="#0c0f11" opacity=".82"/>`)
    .join('');
  const shines = pair('M322 412C326 372 346 326 378 280L388 284C358 330 338 376 334 416Z')
    .map((d) => `<path d="${d}" fill="#fff" opacity=".14"/>`)
    .join('');
  return (
    inlets +
    shines +
    // The airbox intake.
    `<path d="M454 122C462 108 498 108 506 122C510 142 450 142 454 122Z" fill="#0c0f11"/>` +
    // Nose highlight.
    `<path d="M452 470C454 530 462 590 472 632L480 632L470 470Z" fill="#fff" opacity=".12"/>` +
    `<ellipse cx="${CENTRE}" cy="330" rx="52" ry="40" fill="#0d1012"/>` +
    `<circle cx="${CENTRE}" cy="318" r="27" fill="url(#helmet)"/>` +
    `<rect x="456" y="312" width="48" height="14" rx="7" fill="#14191c"/>` +
    mirrors +
    `<path d="M418 372Q420 292 ${CENTRE} 272Q540 292 542 372" fill="none" stroke="#14181b" stroke-width="13" stroke-linecap="round"/>` +
    `<path d="M430 362Q432 300 ${CENTRE} 284" fill="none" stroke="#fff" stroke-opacity=".12" stroke-width="3" stroke-linecap="round"/>` +
    `<line x1="${CENTRE}" y1="272" x2="${CENTRE}" y2="392" stroke="#14181b" stroke-width="12" stroke-linecap="round"/>`
  );
};

const SHADE =
  '<defs>' +
  // Dark toward the edges, so flat zone colours read as rounded.
  '<linearGradient id="shade" x1="0" x2="1" y1="0" y2="0">' +
  '<stop offset="0" stop-color="#000" stop-opacity=".34"/>' +
  '<stop offset=".5" stop-color="#fff" stop-opacity=".1"/>' +
  '<stop offset="1" stop-color="#000" stop-opacity=".34"/>' +
  '</linearGradient>' +
  '<radialGradient id="shadow"><stop offset="0" stop-color="#000" stop-opacity=".5"/>' +
  '<stop offset="1" stop-color="#000" stop-opacity="0"/></radialGradient>' +
  '<linearGradient id="tyre" x1="0" x2="1" y1="0" y2="0">' +
  '<stop offset="0" stop-color="#0b0d0f"/><stop offset=".5" stop-color="#23292d"/><stop offset="1" stop-color="#0b0d0f"/>' +
  '</linearGradient>' +
  '<radialGradient id="helmet" cx=".4" cy=".35"><stop offset="0" stop-color="#eef1f2"/>' +
  '<stop offset="1" stop-color="#8d969b"/></radialGradient>' +
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
