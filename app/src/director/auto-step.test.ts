import { describe, expect, it } from "vitest";
import { previewView } from "../host/preview";
import type { AutoStepView, GameView, Phase } from "../types";
import { autoStepBar } from "./auto-step";

const slide = { id: "q1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;

function viewIn(phase: Phase, autoStep: AutoStepView | null, stepNo = 7): GameView {
  const view = previewView({ ...slide }, 0, 1, "Quiz");
  return { ...view, phase, autoStep, stepNo, settings: { ...view.settings, allAnsweredDelayS: 3, nextAfterRevealDelayS: 5 } };
}

const closed: Phase = { phase: "question", step: "closed" };
const revealed: Phase = { phase: "question", step: "revealed", winner: null };

describe("autoStepBar", () => {
  it("runs over the quiz's whole delay and starts as far down as the wait is", () => {
    expect(autoStepBar(viewIn(closed, { action: "reveal", inMs: 3000 }))).toEqual({ key: "reveal:7", totalMs: 3000, elapsedMs: 0 });
    expect(autoStepBar(viewIn(closed, { action: "reveal", inMs: 1200 }))).toEqual({ key: "reveal:7", totalMs: 3000, elapsedMs: 1800 });
    expect(autoStepBar(viewIn(revealed, { action: "nextSlide", inMs: 4000 }))).toEqual({ key: "nextSlide:7", totalMs: 5000, elapsedMs: 1000 });
  });

  it("keeps one key through a wait, whatever is left, and a new one for the next wait", () => {
    const keys = [3000, 2000, 10].map((inMs) => autoStepBar(viewIn(closed, { action: "reveal", inMs }))?.key);
    expect(new Set(keys).size).toBe(1);
    expect(autoStepBar(viewIn(revealed, { action: "nextSlide", inMs: 5000 }, 8))?.key).not.toBe(keys[0]);
  });

  it("never waits less than what is left, and has no bar without a coming step", () => {
    expect(autoStepBar(viewIn(closed, { action: "reveal", inMs: 9000 }))).toEqual({ key: "reveal:7", totalMs: 9000, elapsedMs: 0 });
    expect(autoStepBar(viewIn(closed, null))).toBeNull();
  });
});
