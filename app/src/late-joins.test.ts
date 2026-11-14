import { describe, expect, it } from "vitest";
import { previewView } from "./host/preview";
import { lateJoins } from "./late-joins";
import type { GameView, Phase, PlayerView } from "./types";

const slide = { id: "b1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;
const closed: Phase = { phase: "question", step: "closed" };

function player(buzzer: number): PlayerView {
  return {
    buzzer, name: "", score: 0, enabled: true, totalAnswerMs: 0, rank: 1, answered: false, answer: null, answerCorrect: null,
    answerMs: null, lockedOut: false, joinsNext: false, delta: null, joker: { on: false, chosen: false, left: 0, canChange: false },
  };
}

function viewOf(phase: Phase, buzzers: number[], stepNo: number): GameView {
  return { ...previewView({ ...slide }, 1, 3, "Quiz"), phase, stepNo, players: buzzers.map(player) };
}

const twenty = Array.from({ length: 20 }, (_, i) => i + 1);

describe("lateJoins", () => {
  it("names a buzzer that joined once the slides began", () => {
    expect(lateJoins(viewOf(closed, twenty, 40), viewOf(closed, [...twenty, 40], 41), new Set())).toEqual([40]);
    expect(lateJoins(viewOf({ phase: "billboard" }, twenty, 40), viewOf({ phase: "billboard" }, [...twenty, 40, 41], 40), new Set())).toEqual([40, 41]);
  });

  it("says nothing for teams registering before the first slide", () => {
    expect(lateJoins(viewOf({ phase: "lobby" }, [1], 2), viewOf({ phase: "lobby" }, [1, 2], 3), new Set())).toEqual([]);
    expect(lateJoins(viewOf({ phase: "welcome" }, [], 1), viewOf({ phase: "welcome" }, [7], 2), new Set())).toEqual([]);
  });

  it("says nothing for a buzzer joined from this window, a show just begun or resumed, or another show", () => {
    expect(lateJoins(viewOf(closed, twenty, 40), viewOf(closed, [...twenty, 21], 41), new Set([21]))).toEqual([]);
    expect(lateJoins(null, viewOf(closed, twenty, 40), new Set())).toEqual([]);
    expect(lateJoins(viewOf(closed, twenty, 40), viewOf(closed, [1, 2], 3), new Set())).toEqual([]);
  });
});
