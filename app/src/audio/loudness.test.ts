import { describe, expect, it } from "vitest";
import { channelWeights, LOUDNESS_RATE, measureLoudness, normalization, type Loudness, type Samples } from "./loudness";

/** `seconds` of a 997 Hz sine at `amplitude` (1 = 0 dBFS). */
function sine(seconds: number, amplitude = 1): Float32Array {
  return Float32Array.from({ length: Math.round(seconds * LOUDNESS_RATE) }, (_, i) => amplitude * Math.sin((2 * Math.PI * 997 * i) / LOUDNESS_RATE));
}

function joined(...parts: Float32Array[]): Float32Array {
  const all = new Float32Array(parts.reduce((n, p) => n + p.length, 0));
  parts.reduce((at, p) => (all.set(p, at), at + p.length), 0);
  return all;
}

const mono = (channel: Float32Array): Samples => ({ sampleRate: LOUDNESS_RATE, channels: [channel] });
const silence = (seconds: number) => new Float32Array(Math.round(seconds * LOUDNESS_RATE));

describe("loudness", () => {
  it("measures a full-scale 997 Hz sine in one channel as -3.01 LUFS, as BS.1770-4 states", () => {
    const { lufs, peakDb, durationMs } = measureLoudness(mono(sine(10)));
    expect(lufs).toBeCloseTo(-3.01, 1);
    expect(peakDb).toBeCloseTo(0, 3);
    expect(durationMs).toBe(10_000);
  });

  it("adds the channels up", () => {
    const tone = sine(10);
    expect(measureLoudness({ sampleRate: LOUDNESS_RATE, channels: [tone, tone] }).lufs).toBeCloseTo(0, 1);
    // 5.1: the low-frequency channel does not count, the surround ones count 1.41 times.
    expect(channelWeights(6)).toEqual([1, 1, 1, 0, 1.41, 1.41]);
    const lfeOnly = [silence(10), silence(10), silence(10), tone, silence(10), silence(10)];
    expect(measureLoudness({ sampleRate: LOUDNESS_RATE, channels: lfeOnly }).lufs).toBeNull();
  });

  it("leaves silence and very quiet stretches out of the measurement", () => {
    // Half the blocks are silent; three are partly loud (75, 50, 25 %), which lowers the mean a little.
    const partly = -3.01 + 10 * Math.log10(48.5 / 50);
    expect(measureLoudness(mono(joined(sine(5), silence(5)))).lufs).toBeCloseTo(partly, 1);
    // -40 dB is below the relative gate (-10 LU under the loudness of all blocks above -70 LUFS).
    expect(measureLoudness(mono(joined(sine(5), sine(5, 0.01)))).lufs).toBeCloseTo(partly, 1);
  });

  it("measures only the part that plays", () => {
    const song = joined(silence(2), sine(2), silence(6));
    expect(measureLoudness(mono(song), { fromMs: 2_000, toMs: 4_000 }).lufs).toBeCloseTo(-3.01, 1);
    expect(measureLoudness(mono(song), { fromMs: 6_000, toMs: null })).toEqual({ lufs: null, peakDb: -Infinity, durationMs: 4_000 });
  });

  it("refuses samples at another rate", () => {
    expect(() => measureLoudness({ sampleRate: 44_100, channels: [sine(1)] })).toThrow();
  });
});

describe("normalization", () => {
  const measured = (lufs: number | null, peakDb: number, durationMs = 30_000): Loudness => ({ lufs, peakDb, durationMs });

  it("raises a quiet sound and lowers a loud one to the target", () => {
    expect(normalization(measured(-20, -10), -16)).toEqual({ status: "changed", gainDb: 4 });
    expect(normalization(measured(-9.96, 0), -16)).toEqual({ status: "changed", gainDb: -6 });
    expect(normalization(measured(-26.04, -12), -23)).toEqual({ status: "changed", gainDb: 3 });
  });

  it("raises a sound only as far as its peaks stay at -1 dBFS", () => {
    expect(normalization(measured(-20, -2), -16)).toEqual({ status: "limited", gainDb: 1 });
    expect(normalization(measured(-20, 0), -16)).toEqual({ status: "limited", gainDb: 0 });
    // Already at the target, it is not lowered for its peaks.
    expect(normalization(measured(-16.2, 0), -16)).toEqual({ status: "atTarget", gainDb: 0 });
  });

  it("keeps the gain within its limits", () => {
    expect(normalization(measured(-60, -50), -16)).toEqual({ status: "limited", gainDb: 20 });
    expect(normalization(measured(20, 20), -16)).toEqual({ status: "limited", gainDb: -30 });
  });

  it("leaves a sound it cannot measure as it is", () => {
    expect(normalization(measured(null, -Infinity), -16)).toEqual({ status: "silent", gainDb: 0 });
    expect(normalization(measured(null, 0, 300), -16)).toEqual({ status: "tooShort", gainDb: 0 });
  });
});
