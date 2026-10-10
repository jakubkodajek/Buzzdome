import { describe, expect, it } from "vitest";
import { timeLeft } from "./answer-time";
import type { ClockView, Phase } from "./types";
import { blurRadius, mosaicColumns } from "./video-effect";

const running: Phase = { phase: "question", step: "running", clock: { limitMs: 20_000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 } };
const clock = (remainingMs: number, running = true): ClockView => ({ limitMs: 20_000, remainingMs, elapsedMs: 20_000 - remainingMs, running, points: 10 });

describe("answer time left", () => {
  it("is all of it before the clock starts and none once the question is over", () => {
    expect(timeLeft({ phase: "question", step: "ready" }, null)).toBe(1);
    expect(timeLeft({ phase: "question", step: "closed" }, null)).toBe(0);
    expect(timeLeft({ phase: "question", step: "revealed", winner: null }, null)).toBe(0);
    expect(timeLeft({ phase: "billboard" }, null)).toBe(0);
  });

  it("runs down with the clock, also between two readings of it", () => {
    expect(timeLeft(running, clock(15_000))).toBe(0.75);
    expect(timeLeft(running, clock(15_000), 5_000)).toBe(0.5);
    expect(timeLeft(running, clock(1_000), 5_000)).toBe(0);
    // A paused clock stands still; an untimed one keeps all its time.
    expect(timeLeft(running, clock(15_000, false), 5_000)).toBe(0.75);
    expect(timeLeft(running, { ...clock(0), limitMs: null, remainingMs: null })).toBe(1);
  });
});

describe("video effect", () => {
  it("draws a mosaic of large blocks that grow finer until they are pixels", () => {
    expect(mosaicColumns(1, 1280)).toBe(12);
    expect(mosaicColumns(0.5, 1280)).toBe(124);
    expect(mosaicColumns(0, 1280)).toBeNull();
    expect(mosaicColumns(0.01, 1280)).toBeGreaterThan(1200);
    // Strength out of range counts as its nearest end; an unknown width draws nothing.
    expect(mosaicColumns(3, 1280)).toBe(12);
    expect(mosaicColumns(1, 0)).toBeNull();
  });

  it("blurs in proportion to the strength and the video's size", () => {
    expect(blurRadius(1, 1000)).toBe(40);
    expect(blurRadius(0.5, 1000)).toBe(20);
    expect(blurRadius(-1, 1000)).toBe(0);
  });
});
