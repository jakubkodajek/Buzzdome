import type { StringKey } from "../i18n";
import type { Command, GameView } from "../types";

/** What answering "Restart the question?" does: a command to give, a notice to show, or nothing. */
export type RestartOutcome = { command: Command } | { notice: StringKey } | null;

/**
 * What the quizmaster's answer to "Restart the question?" does, decided
 * from the show as it is once they answer (`now`): it may have moved on
 * while the dialog was open. `pausedClock` tells whether asking paused the
 * clock: No resumes only a clock it paused, and only while it is still
 * paused. Yes restarts the question if it still can start again, else
 * tells why it does not (`tooLateNotice`).
 */
export function restartOutcome(yes: boolean, pausedClock: boolean, now: GameView | null): RestartOutcome {
  if (!yes) return pausedClock && now?.controls.resumeClock ? { command: { type: "resumeClock" } } : null;
  if (now?.controls.restartQuestion) return { command: { type: "restartQuestion" } };
  return { notice: tooLateNotice(now) };
}

/** Why a question no longer starts again, worded from where the show is now. */
function tooLateNotice(now: GameView | null): StringKey {
  if (now?.phase.phase !== "question") return "restartMovedOn";
  return now.phase.step === "ready" ? "restartAlreadyDone" : "restartTooLate";
}
