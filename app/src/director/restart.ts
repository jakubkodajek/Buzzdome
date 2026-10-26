import type { StringKey } from "../i18n";
import type { Command, GameView } from "../types";

/** What answering "Restart the question?" does: a command to give, a notice to show, or nothing. */
export type RestartOutcome = { command: Command } | { notice: StringKey } | null;

/** The question a restart was asked for: its slide, and whether asking paused its clock. */
export type RestartAsked = { slideIndex: number; pausedClock: boolean };

/**
 * What the quizmaster's answer to "Restart the question?" does, decided
 * from the show as it is once they answer (`now`): it may have moved on
 * while the dialog was open, also to another question (the projector's
 * keys step the show), which the answer never touches. No resumes only a
 * clock that asking paused, and only while it is still paused. Yes
 * restarts the question if it still can start again, else tells why it
 * does not (`tooLateNotice`).
 */
export function restartOutcome(yes: boolean, asked: RestartAsked, now: GameView | null): RestartOutcome {
  const same = now !== null && now.slideIndex === asked.slideIndex ? now : null;
  if (!yes) return asked.pausedClock && same?.controls.resumeClock ? { command: { type: "resumeClock" } } : null;
  if (same?.controls.restartQuestion) return { command: { type: "restartQuestion" } };
  return { notice: tooLateNotice(same) };
}

/** Why the question asked about no longer starts again, worded from where it is now: none once the show left it. */
function tooLateNotice(now: GameView | null): StringKey {
  if (now?.phase.phase !== "question") return "restartMovedOn";
  return now.phase.step === "ready" ? "restartAlreadyDone" : "restartTooLate";
}
