/**
 * Place and session names for the UI (spec section 14): the appendix names, never event titles.
 * The collector's `names.rs` holds the same tables for the recording library; State carries only
 * the IDs, so the dashboard needs its own copy. An unlisted ID has no label.
 */
const TRACKS: Readonly<Record<number, string>> = {
  0: 'Melbourne',
  2: 'Shanghai',
  3: 'Sakhir (Bahrain)',
  4: 'Catalunya',
  5: 'Monaco',
  6: 'Montreal',
  7: 'Silverstone',
  9: 'Hungaroring',
  10: 'Spa',
  11: 'Monza',
  12: 'Singapore',
  13: 'Suzuka',
  14: 'Abu Dhabi',
  15: 'Texas',
  16: 'Brazil',
  17: 'Austria',
  19: 'Mexico',
  20: 'Baku (Azerbaijan)',
  26: 'Zandvoort',
  27: 'Imola',
  29: 'Jeddah',
  30: 'Miami',
  31: 'Las Vegas',
  32: 'Losail',
  39: 'Silverstone (Reverse)',
  40: 'Austria (Reverse)',
  41: 'Zandvoort (Reverse)',
  42: 'Madrid',
};

const SESSIONS: Readonly<Record<number, string>> = {
  1: 'Practice 1',
  2: 'Practice 2',
  3: 'Practice 3',
  4: 'Short Practice',
  5: 'Qualifying 1',
  6: 'Qualifying 2',
  7: 'Qualifying 3',
  8: 'Short Qualifying',
  9: 'One-Shot Qualifying',
  10: 'Sprint Shootout 1',
  11: 'Sprint Shootout 2',
  12: 'Sprint Shootout 3',
  13: 'Short Sprint Shootout',
  14: 'One-Shot Sprint Shootout',
  15: 'Race',
  16: 'Race 2',
  17: 'Race 3',
  18: 'Time Trial',
};

export const trackName = (id: number | undefined): string | undefined => (id === undefined ? undefined : TRACKS[id]);
export const sessionName = (id: number | undefined): string | undefined => (id === undefined ? undefined : SESSIONS[id]);
