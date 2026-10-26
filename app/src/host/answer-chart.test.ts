import { describe, expect, it } from "vitest";
import { DEFAULT_QUIZ_SETTINGS } from "../bindings/Constants";
import { newSlide } from "../editor/slides";
import type { GameView, Phase } from "../types";
import { chartShares } from "./answer-chart";
import { previewView } from "./preview";

const revealed: Phase = { phase: "question", step: "revealed", winner: null };

function shown(phase: Phase, answerChart: boolean): GameView {
  return {
    ...previewView(newSlide("question", "q"), 0, 1, "Quiz"),
    phase,
    answerCounts: [1, 2, 0, 0],
    results: { right: 2, wrong: 1, none: 1 },
    settings: { ...DEFAULT_QUIZ_SETTINGS, answerChart },
  };
}

describe("chartShares", () => {
  it("gives each answer's share of every player the question counted, those with no answer too", () => {
    expect(chartShares(shown(revealed, true))).toEqual([0.25, 0.5, 0, 0]);
  });

  it("draws nothing unless the setting is on and the answer is shown", () => {
    expect(chartShares(shown(revealed, false))).toBeNull();
    expect(chartShares(shown({ phase: "question", step: "closed" }, true))).toBeNull();
    expect(chartShares({ ...shown(revealed, true), answerCounts: null, results: null })).toBeNull();
  });
});
