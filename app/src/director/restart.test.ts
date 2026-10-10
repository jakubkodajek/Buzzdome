import { describe, expect, it } from "vitest";
import { NO_CONTROLS, previewView } from "../host/preview";
import type { Controls, GameView, Phase } from "../types";
import { restartOutcome } from "./restart";

const slide = { id: "q1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;
const clock = { limitMs: 30_000, elapsedBeforeMs: 4_000, runningSinceMs: null, pressesFromMs: 0 };

function viewIn(phase: Phase, controls: Partial<Controls> = {}): GameView {
  return { ...previewView({ ...slide }, 0, 1, "Quiz"), phase, controls: { ...NO_CONTROLS, ...controls } };
}

const running = viewIn({ phase: "question", step: "running", clock }, { pauseClock: true, restartQuestion: true });
const paused = viewIn({ phase: "question", step: "running", clock }, { resumeClock: true, restartQuestion: true });

describe("restartOutcome", () => {
  it("restarts a question that can still start again", () => {
    expect(restartOutcome(true, true, paused)).toEqual({ command: { type: "restartQuestion" } });
    expect(restartOutcome(true, false, running)).toEqual({ command: { type: "restartQuestion" } });
  });

  it("on No resumes only a clock that asking paused and that is still paused", () => {
    expect(restartOutcome(false, true, paused)).toEqual({ command: { type: "resumeClock" } });
    expect(restartOutcome(false, false, paused)).toBeNull();
    expect(restartOutcome(false, true, running)).toBeNull();
    expect(restartOutcome(false, true, null)).toBeNull();
  });

  it("tells why the question no longer starts again, from where the show is", () => {
    const closed = viewIn({ phase: "question", step: "closed" }, { next: true });
    const revealed = viewIn({ phase: "question", step: "revealed", winner: null }, { next: true });
    // Another dialog's Yes restarted it already.
    const ready = viewIn({ phase: "question", step: "ready" }, { next: true });
    expect(restartOutcome(true, true, closed)).toEqual({ notice: "restartTooLate" });
    expect(restartOutcome(true, true, revealed)).toEqual({ notice: "restartTooLate" });
    expect(restartOutcome(true, true, ready)).toEqual({ notice: "restartAlreadyDone" });
    expect(restartOutcome(true, true, viewIn({ phase: "billboard" }, { next: true }))).toEqual({ notice: "restartMovedOn" });
    expect(restartOutcome(true, true, null)).toEqual({ notice: "restartMovedOn" });
  });
});
