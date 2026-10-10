import { afterEach, describe, expect, it, vi } from "vitest";
import { gainChange, readLoudnessTarget, writeLoudnessTarget } from "./normalize";

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
});
