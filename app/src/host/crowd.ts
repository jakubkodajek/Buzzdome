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

/** How many buzzes the projector's queue names at most; the rest are counted. */
export const MAX_QUEUE_TURNS = 3;

/** A buzz of the queue, as far as the strip needs to know. */
interface QueueTurn {
  state: "answering" | "wrong" | "right" | "waiting";
}

export interface QueueStrip<T> {
  /** The buzzes that get a place, in the order they came. */
  shown: T[];
  /** How many more there are than places. */
  more: number;
}

/**
 * The buzzes the queue names while many came: the one answering (or the
 * last one judged) first, then the wrong ones before it and the ones
 * waiting after it, the wrong ones first, `MAX_QUEUE_TURNS` in all. The rest
 * are counted, so the strip keeps to the screen.
 */
export function queueStrip<T extends QueueTurn>(turns: readonly T[]): QueueStrip<T> {
  if (turns.length <= MAX_QUEUE_TURNS) return { shown: [...turns], more: 0 };
  const lit = turns.findIndex((turn) => turn.state === "answering" || turn.state === "right");
  const pivot = lit >= 0 ? lit : turns.length - 1;
  const chosen = new Set([pivot]);
  const before = Array.from({ length: pivot }, (_, i) => pivot - 1 - i);
  const after = Array.from({ length: turns.length - pivot - 1 }, (_, i) => pivot + 1 + i);
  // The closest wrong ones and the next waiting, evenly, then whatever still has room.
  const order = [before[0], after[0], before[1], after[1], ...before.slice(2), ...after.slice(2)];
  for (const i of order) {
    if (chosen.size >= MAX_QUEUE_TURNS) break;
    if (i !== undefined) chosen.add(i);
  }
  return { shown: turns.filter((_, i) => chosen.has(i)), more: turns.length - chosen.size };
}

/** Names longer than this many characters are drawn smaller, on two lines, in the leaderboard. */
export const LEADERBOARD_LONG_NAME = 22;
