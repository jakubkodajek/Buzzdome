import type { ClockView, Phase } from "./types";

/**
 * Share of a question's answer time still left, 1 to 0, `sinceMs` after
 * `clock` was read: all of it before the clock starts and on an untimed
 * question, none once the question is over or on a slide that is no
 * question. The clock gauge shows it; a video effect fades with it.
 */
export function timeLeft(phase: Phase, clock: ClockView | null, sinceMs = 0): number {
  if (phase.phase !== "question" || phase.step === "closed" || phase.step === "revealed") return 0;
  if (!clock || clock.remainingMs === null || !clock.limitMs) return 1;
  const remaining = clock.remainingMs - (clock.running ? sinceMs : 0);
  return Math.min(1, Math.max(0, remaining / clock.limitMs));
}
