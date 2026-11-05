// Writes crates/livery-art/tests/fixtures/web-liveries.json: what the web build's livery
// choice and fallback car give for every team ID, so the native crate can be checked
// against it. Run from the repository root: `node tools/livery-art/golden.mts`.
import { writeFileSync } from 'node:fs';
import { chooseLivery, type Rgb } from '../../dashboard/src/liveries/choose.ts';
import { fallbackCarSvg } from '../../dashboard/src/liveries/fallbackCar.ts';

// The owner's 21-car pack (spec section 13): the 2025 grid 0-9 and the 2026 grid 476-486.
const PACK = [
  ...Array.from({ length: 10 }, (_, id) => `${id}.png`),
  ...Array.from({ length: 11 }, (_, i) => `${476 + i}.png`),
].sort();
const FILE_SETS: Record<string, string[]> = {
  empty: [],
  pack: PACK,
  'pack+my-team': [...PACK, 'my-team.png'].sort(),
};

// Every team ID a u8 game field can carry, the 2026 grid, and two wide IDs.
const TEAM_IDS = [
  ...Array.from({ length: 256 }, (_, id) => id),
  ...Array.from({ length: 11 }, (_, i) => 476 + i),
  1000,
  65535,
];

/** Distinct, deterministic game colours for a team. */
function gameColours(teamId: number): Rgb[] {
  return [0, 1, 2, 3].map((slot) => ({
    r: (teamId * 37 + slot * 61) % 256,
    g: (teamId * 91 + slot * 17 + 5) % 256,
    b: (teamId * 13 + slot * 101 + 9) % 256,
  }));
}

/** FNV-1a 64 over the UTF-8 bytes, as hex. */
function fnv1a64(text: string): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, '0');
}

const hex = (c: Rgb) => [c.r, c.g, c.b].map((v) => v.toString(16).padStart(2, '0')).join('');

// One row per case: [set, teamId, myTeam, numColours, file or null, the four resolved
// colours as one hex string, FNV-1a 64 of the fallback car SVG for those colours].
// Game colours are `gameColours(teamId)`, and `numColours` is `teamId % 6`.
const rows: unknown[] = [];
for (const set of Object.keys(FILE_SETS)) {
  const ids = set === 'empty' ? TEAM_IDS.filter((id) => id < 10 || (id >= 476 && id <= 486)) : TEAM_IDS;
  for (const teamId of ids) {
    for (const myTeam of [0, 1]) {
      const numColours = teamId % 6;
      const choice = chooseLivery({ teamId, myTeam, numColours, colours: gameColours(teamId) }, FILE_SETS[set]);
      rows.push([
        set,
        teamId,
        myTeam,
        numColours,
        choice.source === 'pack' ? choice.file : null,
        choice.colours.map(hex).join(''),
        fnv1a64(fallbackCarSvg(choice.colours)),
      ]);
    }
  }
}
const none = chooseLivery(undefined, PACK);
const golden = {
  fileSets: FILE_SETS,
  noParticipant: [none.colours.map(hex).join(''), fnv1a64(fallbackCarSvg(none.colours))],
  rows,
};
writeFileSync('crates/livery-art/tests/fixtures/web-liveries.json', JSON.stringify(golden).replaceAll('],[', '],\n[') + '\n');
console.log(`wrote ${rows.length} cases`);
