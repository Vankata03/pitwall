/** An sRGB colour as the game sends it, 0 to 255 per channel. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** The fields of a car's Participants entry the livery choice reads. */
export interface LiveryCar {
  teamId: number;
  /** `m_myTeam`: non-zero for the player's custom team. */
  myTeam: number;
  /** `m_numColours`: how many of `colours` the game filled in. */
  numColours: number;
  /** `m_liveryColour`: four RGB colours. */
  colours: readonly Rgb[];
}

export type LiveryChoice =
  | { source: 'pack'; file: string; url: string; colours: readonly Rgb[] }
  | { source: 'fallback'; colours: readonly Rgb[] };

/** Livery colour slots the fallback car recolours (see `fallbackCar.ts`). */
export const LIVERY_SLOTS = 4;

/** Last resort when the game sends no colour for a slot. */
export const NEUTRAL_COLOURS: readonly Rgb[] = [
  { r: 154, g: 167, b: 173 },
  { r: 232, g: 237, b: 239 },
  { r: 90, g: 100, b: 105 },
  { r: 27, g: 31, b: 33 },
];

/** Game colours first, in game order; a slot the game did not send is neutral. */
function resolveColours(car: LiveryCar): Rgb[] {
  const sent = Math.max(0, Math.min(Math.trunc(car.numColours) || 0, LIVERY_SLOTS));
  return NEUTRAL_COLOURS.map((neutral, slot) => (slot < sent ? (car.colours[slot] ?? neutral) : neutral));
}

/**
 * Pick the picture for a car, given the file names from `GET /api/liveries`: `my-team.png`
 * for the player's custom team, else `<teamId>.png`, else the fallback car. A name that
 * matches neither pattern exactly is ignored.
 */
export function chooseLivery(car: LiveryCar | undefined, files: readonly string[]): LiveryChoice {
  if (!car) return { source: 'fallback', colours: NEUTRAL_COLOURS };
  const colours = resolveColours(car);
  const wanted = [...(car.myTeam !== 0 ? ['my-team.png'] : []), `${Math.trunc(car.teamId)}.png`];
  const file = wanted.find((name) => files.includes(name));
  if (file === undefined) return { source: 'fallback', colours };
  return { source: 'pack', file, url: `/liveries/${file}`, colours };
}
