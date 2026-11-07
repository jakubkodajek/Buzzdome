import { describe, expect, it } from "vitest";
import { previewView } from "../host/preview";
import type { GameView, Phase } from "../types";
import { askingOf } from "./asking";

const slide = { id: "q1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;
const clock = { limitMs: 30_000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 };

function viewIn(phase: Phase, stepNo: number): GameView {
  return { ...previewView({ ...slide }, 0, 1, "Quiz"), phase, stepNo };
}

const ready: Phase = { phase: "question", step: "ready" };
const running: Phase = { phase: "question", step: "running", clock };
const revealed: Phase = { phase: "question", step: "revealed", winner: null };

/** The asking each view of a show is in. */
function askings(views: GameView[]): (number | null)[] {
  let last: number | null = null;
  return views.map((view) => (last = askingOf(last, view)));
}

describe("askingOf", () => {
  it("keeps one asking from ready through the answer", () => {
    expect(askings([viewIn(ready, 4), viewIn(running, 5), viewIn(running, 5), viewIn(revealed, 6)])).toEqual([4, 4, 4, 4]);
  });

  it("starts a new asking when the question is restarted on the same slide", () => {
    expect(askings([viewIn(ready, 4), viewIn(running, 5), viewIn(revealed, 6), viewIn(ready, 7), viewIn(running, 8)])).toEqual([4, 4, 4, 7, 7]);
  });

  it("has none until a question is seen ready", () => {
    expect(askings([viewIn(running, 5), viewIn(revealed, 6)])).toEqual([null, null]);
    expect(askingOf(3, null)).toBe(3);
  });
});
