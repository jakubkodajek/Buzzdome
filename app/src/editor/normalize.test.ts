import { afterEach, describe, expect, it, vi } from "vitest";
import { readsWhole, SoundTooLong } from "../audio/decode";
import { gainAfterTrim, gainChange, readLoudnessTarget, unreadSound, writeLoudnessTarget } from "./normalize";

describe("normalize", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("remembers the chosen target and starts at Standard", () => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value) });
    expect(readLoudnessTarget()).toBe(-16);
    writeLoudnessTarget(-23);
    expect(readLoudnessTarget()).toBe(-23);
    // A value no target has, written by hand or by an older version, falls back.
    store.set("quiz-show.loudness-target", "-99");
    expect(readLoudnessTarget()).toBe(-16);
  });

  it("changes the gain only when it differs from what the measurement asks for", () => {
    expect(gainChange(0, { gainDb: 4.5, status: "changed" })).toEqual({ gainDb: 4.5, status: "changed", message: "normalizeDone" });
    expect(gainChange(0, { gainDb: 2, status: "limited" })).toEqual({ gainDb: 2, status: "limited", message: "normalizeLimited" });
    // A sound already at the target loses a gain it had from before: that is a change.
    expect(gainChange(3, { gainDb: 0, status: "atTarget" })).toEqual({ gainDb: 0, status: "changed", message: "normalizeDone" });
    // One normalized before is at the target now.
    expect(gainChange(4.5, { gainDb: 4.5, status: "changed" })).toEqual({ gainDb: null, status: "atTarget", message: "loudnessAtTarget" });
    expect(gainChange(2, { gainDb: 2, status: "limited" })).toEqual({ gainDb: null, status: "limited", message: "loudnessLimited" });
    expect(gainChange(3, { gainDb: 0, status: "silent" })).toEqual({ gainDb: null, status: "silent", message: "loudnessSilent" });
    expect(gainChange(3, { gainDb: 0, status: "tooShort" })).toEqual({ gainDb: null, status: "tooShort", message: "loudnessTooShort" });
  });

  it("measures a normalized sound's new part again, keeping its gain where the part cannot be measured", () => {
    expect(gainAfterTrim(3, { lufs: -20, peakDb: -10, durationMs: 5_000 }, -16)).toBe(4);
    expect(gainAfterTrim(3, { lufs: null, peakDb: -Infinity, durationMs: 5_000 }, -16)).toBe(3);
    expect(gainAfterTrim(3, { lufs: null, peakDb: -20, durationMs: 100 }, -16)).toBe(3);
  });

  it("reads a sound whole up to ten minutes, and one that tells no length", () => {
    expect(readsWhole(10 * 60_000)).toBe(true);
    expect(readsWhole(10 * 60_000 + 1)).toBe(false);
    expect(readsWhole(Number.NaN)).toBe(true);
    expect(unreadSound(new SoundTooLong("media/long.mp3"))).toBe("tooLong");
    expect(unreadSound(new Error("media/song.mp3: HTTP 404"))).toBe("failed");
  });
});
