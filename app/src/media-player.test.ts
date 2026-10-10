import { describe, expect, it } from "vitest";
import { MAX_BOARD_COLUMNS } from "./bindings/Constants";
import type { LibraryFile, NowPlaying, SoundBoard } from "./types";
import { msToEnd, padColor, positionAt, progressOf, searchFiles, shownRows } from "./media-player";

const SONG: NowPlaying = { id: 1, reference: "media/song.mp3", kind: "audio", positionMs: 2_000, sinceMs: 10_000 };
const PAUSED: NowPlaying = { ...SONG, sinceMs: null };

describe("media player", () => {
  it("counts the position on from where the file last started", () => {
    expect(positionAt(SONG, 10_500)).toBe(2_500);
    expect(positionAt(PAUSED, 99_000)).toBe(2_000);
    // A clock read just before the start does not go back.
    expect(positionAt(SONG, 9_990)).toBe(2_000);
  });

  it("lays the shown pads out row by row in their places of the largest grid", () => {
    const board = { rows: 3, columns: 4 } as SoundBoard;
    expect(shownRows(board)).toEqual([
      [0, 1, 2, 3],
      [MAX_BOARD_COLUMNS, MAX_BOARD_COLUMNS + 1, MAX_BOARD_COLUMNS + 2, MAX_BOARD_COLUMNS + 3],
      [2 * MAX_BOARD_COLUMNS, 2 * MAX_BOARD_COLUMNS + 1, 2 * MAX_BOARD_COLUMNS + 2, 2 * MAX_BOARD_COLUMNS + 3],
    ]);
  });

  it("colours a pad by its column unless it has a colour", () => {
    const pad = { source: null, label: "", color: null };
    expect(padColor(pad, 0)).toBe("red");
    expect(padColor(pad, MAX_BOARD_COLUMNS + 1)).toBe(padColor(pad, 1));
    expect(padColor({ ...pad, color: "teal" }, 0)).toBe("teal");
  });

  it("finds files by any part of their name, whatever its case", () => {
    const files: LibraryFile[] = [
      { reference: "media/Potlesk.mp3", kind: "audio" },
      { reference: "media/Fanfára.wav", kind: "audio" },
      { reference: "media/intro.mp4", kind: "video" },
    ];
    expect(searchFiles(files, "  FANF ").map((f) => f.reference)).toEqual(["media/Fanfára.wav"]);
    expect(searchFiles(files, "media")).toEqual([]);
    expect(searchFiles(files, "")).toEqual(files);
  });

  it("shows how far the playing file has come and what is left of it", () => {
    expect(progressOf("media/other.mp3", SONG, 8_000, 11_000)).toEqual({ state: "idle" });
    expect(progressOf(null, SONG, 8_000, 11_000)).toEqual({ state: "idle" });
    expect(progressOf(SONG.reference, SONG, 8_000, 12_000)).toEqual({ state: "playing", remainingMs: 4_000, done: 0.5 });
    expect(progressOf(SONG.reference, PAUSED, 8_000, 12_000)).toEqual({ state: "paused", remainingMs: 6_000, done: 0.25 });
    expect(progressOf(SONG.reference, SONG, undefined, 12_000)).toEqual({ state: "playing", remainingMs: null, done: null });
    expect(progressOf(SONG.reference, SONG, 3_000, 60_000)).toEqual({ state: "playing", remainingMs: 0, done: 1 });
  });

  it("knows when a sound or video ends, but not a picture or a paused file", () => {
    expect(msToEnd(SONG, 8_000, 11_000)).toBe(5_000);
    expect(msToEnd(SONG, 8_000, 99_000)).toBe(0);
    expect(msToEnd(PAUSED, 8_000, 11_000)).toBeNull();
    expect(msToEnd({ ...SONG, kind: "image" }, 8_000, 11_000)).toBeNull();
    expect(msToEnd(SONG, undefined, 11_000)).toBeNull();
    expect(msToEnd(SONG, Number.NaN, 11_000)).toBeNull();
    expect(msToEnd(null, 8_000, 11_000)).toBeNull();
  });
});
