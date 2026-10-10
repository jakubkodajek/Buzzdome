// What the media player shows (spec SN-7): the Director's search, sound
// board and countdowns, and where a video on the projector stands.

import { MAX_BOARD_COLUMNS } from "./bindings/Constants";
import { mediaFileName } from "./media";
import type { LibraryFile, MediaNow, NowPlaying, Pad, PadColor, SoundBoard } from "./types";

/** The colours a pad can have, and the ones its column gives it when it has none of its own. */
export const PAD_COLORS: readonly PadColor[] = ["red", "orange", "yellow", "green", "teal", "blue", "purple", "pink"];

/**
 * What the media player plays, as a window heard it: the show clock read
 * `atMs` when the window's monotonic clock (`localNow`) read `heardAt`. The window counts on from
 * there on that monotonic clock, never on its wall clock, which may stand
 * apart from the show clock by any amount (G2).
 */
export interface HeardMedia extends MediaNow {
  heardAt: number;
}

/** The clock a window counts on with; the only place it is read, so the windows cannot disagree. */
const localNow = () => performance.now();

/** `media` as heard at `heardAt` (by `localNow`). */
export function heard(media: MediaNow, heardAt: number): HeardMedia {
  return { ...media, heardAt };
}

/** `media` as heard now. */
export function heardNow(media: MediaNow): HeardMedia {
  return heard(media, localNow());
}

/** The show clock at `localMs` (by `localNow`), counted on from what was heard. */
export function showClockAt(media: HeardMedia, localMs: number): number {
  return media.atMs + Math.max(0, localMs - media.heardAt);
}

/** The show clock now, counted on from what was heard. */
export function showClockNow(media: HeardMedia): number {
  return showClockAt(media, localNow());
}

/**
 * Where `now` is at `atMs` on the show clock. The same rule as
 * `NowPlaying::position_at` in quiz-core, which the Director needs many
 * times a second for the pads.
 */
export function positionAt(now: NowPlaying, atMs: number): number {
  return now.positionMs + (now.sinceMs === null ? 0 : Math.max(0, atMs - now.sinceMs));
}

/** The places of the pads the grid shows, row by row (see `SoundBoard.pads`). */
export function shownRows(board: SoundBoard): number[][] {
  return Array.from({ length: board.rows }, (_, row) => Array.from({ length: board.columns }, (_, column) => row * MAX_BOARD_COLUMNS + column));
}

/** The pad's colour: its own, or else its column's. */
export function padColor(pad: Pad, place: number): PadColor {
  return pad.color ?? PAD_COLORS[(place % MAX_BOARD_COLUMNS) % PAD_COLORS.length];
}

/** The library file on a pad, if it holds one. */
export function padFile(pad: Pad): string | null {
  return pad.source?.type === "file" ? pad.source.reference : null;
}

/** The files whose name holds `query`, whatever its case; all of them for an empty query. */
export function searchFiles(files: readonly LibraryFile[], query: string): LibraryFile[] {
  const wanted = query.trim().toLocaleLowerCase();
  return wanted ? files.filter((file) => mediaFileName(file.reference).toLocaleLowerCase().includes(wanted)) : [...files];
}

/** How far the file `reference` has played, as its pads and its list row show it. */
export type Progress =
  | { state: "idle" }
  | {
      state: "playing" | "paused";
      /** What is left of a file whose length is known. */
      remainingMs: number | null;
      /** How much of it has played, 0 to 1, when its length is known. */
      done: number | null;
    };

/** How far `reference` has played at `atMs`, given what plays and the file's length. */
export function progressOf(reference: string | null, now: NowPlaying | null, durationMs: number | undefined, atMs: number): Progress {
  if (reference === null || now?.reference !== reference) return { state: "idle" };
  const state = now.sinceMs === null ? "paused" : "playing";
  if (durationMs === undefined || !(durationMs > 0)) return { state, remainingMs: null, done: null };
  const position = Math.min(positionAt(now, atMs), durationMs);
  return { state, remainingMs: durationMs - position, done: position / durationMs };
}

/**
 * In how many ms what plays reaches its end, if it plays and its length is
 * known; a picture has no end. The Director then tells the backend, which
 * stops it whichever window plays it (the projector may be closed).
 */
export function msToEnd(now: NowPlaying | null, durationMs: number | undefined, atMs: number): number | null {
  if (!now || now.sinceMs === null || now.kind === "image" || durationMs === undefined || !(durationMs > 0)) return null;
  return Math.max(0, durationMs - positionAt(now, atMs));
}
