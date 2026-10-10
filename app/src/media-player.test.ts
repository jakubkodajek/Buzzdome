import { afterEach, describe, expect, it, vi } from "vitest";
import { MAX_BOARD_COLUMNS } from "./bindings/Constants";
import type { LibraryFile, NowPlaying, SoundBoard } from "./types";
import { heard, heardNow, msToEnd, overAtCheck, padColor, positionAt, progressOf, searchFiles, showClockAt, showClockNow, shownRows } from "./media-player";
// The cases quiz-core tests `NowPlaying::position_at` with.
import POSITION_CASES from "../../crates/quiz-core/tests/data/position-at.json";

const SONG: NowPlaying = { id: 1, reference: "media/song.mp3", kind: "audio", positionMs: 2_000, sinceMs: 10_000 };
const PAUSED: NowPlaying = { ...SONG, sinceMs: null };

describe("media player", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("counts on the show clock from what the window heard, never on the wall clock", () => {
    // The show clock and the window's clocks read nothing alike; the wall clock is not read at all.
    vi.spyOn(Date, "now").mockImplementation(() => {
      throw new Error("the wall clock was read");
    });
    const media = heard({ now: SONG, atMs: 10_200 }, 777_000);
    expect(showClockAt(media, 777_000)).toBe(10_200);
    expect(showClockAt(media, 777_300)).toBe(10_500);
    expect(positionAt(SONG, showClockAt(media, 777_300))).toBe(2_500);
    // A frame drawn before the event was taken in does not count back.
    expect(showClockAt(media, 776_990)).toBe(10_200);
  });

  it("hears and counts on by the window's monotonic clock alone", () => {
    vi.spyOn(Date, "now").mockImplementation(() => {
      throw new Error("the wall clock was read");
    });
    const local = vi.spyOn(performance, "now").mockReturnValue(777_000);
    const media = heardNow({ now: SONG, atMs: 10_200 });
    expect(media.heardAt).toBe(777_000);
    local.mockReturnValue(777_300);
    expect(showClockNow(media)).toBe(10_500);
  });

  it("counts the position as the core does", () => {
    expect(POSITION_CASES.length).toBeGreaterThan(0);
    for (const { name, positionMs, sinceMs, atMs, expected } of POSITION_CASES) expect(positionAt({ ...SONG, positionMs, sinceMs }, atMs), name).toBe(expected);
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

  it("at its end ends a video, a stopped or failed sound, and one that stands still", () => {
    const sound = { playing: true, failed: false, positionMs: 8_000 };
    expect(overAtCheck(null, null)).toBe(true);
    expect(overAtCheck({ ...sound, playing: false }, null)).toBe(true);
    expect(overAtCheck({ ...sound, failed: true }, null)).toBe(true);
    // Still playing at its length (it started late): it ends by itself, so it is checked again.
    expect(overAtCheck(sound, null)).toBe(false);
    expect(overAtCheck({ ...sound, positionMs: 9_500 }, 8_000)).toBe(false);
    expect(overAtCheck(sound, 8_000)).toBe(true);
  });
});
