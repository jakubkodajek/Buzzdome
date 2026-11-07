// Screens that list every team, laid out for a full event (about 50
// buzzers): the strip of buzzers still to answer, and the leaderboard's
// pages.

/** How many cells the waiting strip holds at most; past that the last one counts the rest. */
export const MAX_WAITING_CELLS = 52;

/** The strip's rows for a number of cells: more rows of smaller cells as more wait, so it keeps to its place in the bar. */
const WAITING_ROWS: readonly { upTo: number; rows: 2 | 3 | 4 }[] = [
  { upTo: 20, rows: 2 },
  { upTo: 33, rows: 3 },
  { upTo: MAX_WAITING_CELLS, rows: 4 },
];

export interface WaitingStrip {
  rows: 2 | 3 | 4;
  /** How many buzzers get a cell. */
  shown: number;
  /** How many more wait than have a cell (the last cell says so). */
  more: number;
}

/** How the strip shows `waiting` buzzers: every one while they fit, else all but a last cell that counts the rest. */
export function waitingStrip(waiting: number): WaitingStrip {
  const shown = waiting > MAX_WAITING_CELLS ? MAX_WAITING_CELLS - 1 : waiting;
  const cells = shown + (waiting > shown ? 1 : 0);
  const rows = WAITING_ROWS.find((size) => cells <= size.upTo)?.rows ?? 4;
  return { rows, shown, more: waiting - shown };
}

/** Rows in a leaderboard column, and columns on a page. */
export const LEADERBOARD_ROWS = 10;
export const LEADERBOARD_COLUMNS = 2;

/** How long the leaderboard shows a page before the next, when the teams take more than one. */
export const LEADERBOARD_PAGE_MS = 10_000;

/** `items` in pieces of `size`, the last one shorter; none for no items. */
export function chunks<T>(items: readonly T[], size: number): T[][] {
  return Array.from({ length: Math.ceil(items.length / size) }, (_, i) => items.slice(i * size, (i + 1) * size));
}

/** The leaderboard's pages, each a list of columns of rows, in ranking order. */
export function leaderboardPages<T>(ranked: readonly T[]): T[][][] {
  return chunks(ranked, LEADERBOARD_ROWS * LEADERBOARD_COLUMNS).map((page) => chunks(page, LEADERBOARD_ROWS));
}
