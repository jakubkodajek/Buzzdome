import { describe, expect, it } from "vitest";
import { inSpan, linearGain, sameSpan, WHOLE_FILE } from "./clip-player";

describe("clip player", () => {
  it("multiplies volume and loudness gain", () => {
    expect(linearGain(100, 0)).toBe(1);
    expect(linearGain(50, 0)).toBe(0.5);
    // +6.02 dB doubles the amplitude, -20 dB is a tenth.
    expect(linearGain(100, 6.0206)).toBeCloseTo(2, 4);
    expect(linearGain(80, -20)).toBeCloseTo(0.08, 6);
    expect(linearGain(0, 20)).toBe(0);
  });

  it("knows which times a part covers", () => {
    const chorus = { fromMs: 15_000, toMs: 45_000 };
    expect([14_999, 15_000, 44_999, 45_000].map((ms) => inSpan(chorus, ms))).toEqual([false, true, true, false]);
    expect(inSpan(WHOLE_FILE, 3_600_000)).toBe(true);
    expect(sameSpan(chorus, { fromMs: 15_000, toMs: 45_000 })).toBe(true);
    expect(sameSpan(chorus, WHOLE_FILE)).toBe(false);
  });
});
