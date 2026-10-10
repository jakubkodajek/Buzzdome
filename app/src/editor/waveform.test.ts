import { describe, expect, it } from "vitest";
import { panned, waveformPeaks, zoomed, MIN_WINDOW_MS } from "./waveform";

describe("waveform", () => {
  it("takes the lowest and highest sample of each column over all channels", () => {
    // 1 kHz, so 1 ms is one sample.
    const left = Float32Array.from([0.1, -0.5, 0.2, 0.9, 0, 0, -0.3, 0.4]);
    const right = Float32Array.from([0, 0, -0.8, 0, 0, 0.6, 0, 0]);
    const peaks = waveformPeaks({ sampleRate: 1000, channels: [left, right] }, { fromMs: 0, toMs: 8 }, 4);
    expect(Array.from(peaks.min)).toEqual([-0.5, -0.8, 0, -0.3].map(Math.fround));
    expect(Array.from(peaks.max)).toEqual([0.1, 0.9, 0.6, 0.4].map(Math.fround));
    // More columns than samples: each column shows the sample under it.
    expect(Array.from(waveformPeaks({ sampleRate: 1000, channels: [left] }, { fromMs: 2, toMs: 4 }, 4).max)).toEqual([0.2, 0.2, 0.9, 0.9].map(Math.fround));
  });

  it("zooms around the pointer and stays inside the sound", () => {
    const all = { fromMs: 0, toMs: 60_000 };
    expect(zoomed(all, 30_000, 0.5, 60_000)).toEqual({ fromMs: 15_000, toMs: 45_000 });
    expect(zoomed(all, 0, 0.5, 60_000)).toEqual({ fromMs: 0, toMs: 30_000 });
    expect(zoomed({ fromMs: 0, toMs: 30_000 }, 0, 4, 60_000)).toEqual(all);
    expect(zoomed(all, 10_000, 0.000001, 60_000).toMs - zoomed(all, 10_000, 0.000001, 60_000).fromMs).toBe(MIN_WINDOW_MS);
  });

  it("pans without leaving the sound", () => {
    const view = { fromMs: 10_000, toMs: 20_000 };
    expect(panned(view, 5_000, 60_000)).toEqual({ fromMs: 15_000, toMs: 25_000 });
    expect(panned(view, -50_000, 60_000)).toEqual({ fromMs: 0, toMs: 10_000 });
    expect(panned(view, 99_000, 60_000)).toEqual({ fromMs: 50_000, toMs: 60_000 });
  });
});
