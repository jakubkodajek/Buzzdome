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

const asked = { slideIndex: 0, pausedClock: true };
const askedRunning = { slideIndex: 0, pausedClock: false };

describe("restartOutcome", () => {
  it("restarts a question that can still start again", () => {
    expect(restartOutcome(true, asked, paused)).toEqual({ command: { type: "restartQuestion" } });
    expect(restartOutcome(true, askedRunning, running)).toEqual({ command: { type: "restartQuestion" } });
  });

  it("on No resumes only a clock that asking paused and that is still paused", () => {
    expect(restartOutcome(false, asked, paused)).toEqual({ command: { type: "resumeClock" } });
    expect(restartOutcome(false, askedRunning, paused)).toBeNull();
    expect(restartOutcome(false, asked, running)).toBeNull();
    expect(restartOutcome(false, asked, null)).toBeNull();
  });

  it("tells why the question no longer starts again, from where the show is", () => {
    const closed = viewIn({ phase: "question", step: "closed" }, { next: true });
    const revealed = viewIn({ phase: "question", step: "revealed", winner: null }, { next: true });
    // Another dialog's Yes restarted it already.
    const ready = viewIn({ phase: "question", step: "ready" }, { next: true });
    expect(restartOutcome(true, asked, closed)).toEqual({ notice: "restartTooLate" });
    expect(restartOutcome(true, asked, revealed)).toEqual({ notice: "restartTooLate" });
    expect(restartOutcome(true, asked, ready)).toEqual({ notice: "restartAlreadyDone" });
    expect(restartOutcome(true, asked, viewIn({ phase: "billboard" }, { next: true }))).toEqual({ notice: "restartMovedOn" });
    expect(restartOutcome(true, asked, null)).toEqual({ notice: "restartMovedOn" });
  });

  it("never touches another question the show moved on to meanwhile", () => {
    const next = (phase: Phase, controls: Partial<Controls>) => ({ ...viewIn(phase, controls), slideIndex: 1 });
    expect(restartOutcome(true, asked, next({ phase: "question", step: "ready" }, { next: true }))).toEqual({ notice: "restartMovedOn" });
    expect(restartOutcome(true, asked, next({ phase: "question", step: "running", clock }, { resumeClock: true, restartQuestion: true }))).toEqual({
      notice: "restartMovedOn",
    });
    expect(restartOutcome(false, asked, next({ phase: "question", step: "running", clock }, { resumeClock: true, restartQuestion: true }))).toBeNull();
  });
});
