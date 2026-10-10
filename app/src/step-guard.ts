// One step of the show per key press or click. A double press, a double
// click or a press made before the new step is on screen must not run the
// step after it unseen (A4): a Space too many on a question starts its clock
// and closes it with no answers.

import type { Command } from "./types";

/** Commands that move the show on, each meant for the step the quizmaster saw. */
const STEP_COMMANDS: ReadonlySet<Command["type"]> = new Set([
  "next",
  "goTo",
  "goToFinal",
  "closeQuestion",
  "restartQuestion",
  "judgeRight",
  "judgeWrong",
  "pauseClock",
  "resumeClock",
  "playVideo",
  "showLeaderboard",
]);

/** How long after a step command another one is dropped: less than a person takes to see the new step and react to it. */
export const STEP_PAUSE_MS = 300;

export interface StepGuard {
  /**
   * Sends `command` with `send`, unless it moves the show on while the last
   * such command is unanswered or less than `STEP_PAUSE_MS` after it: it is
   * dropped then, and the result is null.
   */
  send(command: Command, send: () => Promise<void>): Promise<void> | null;
}

export function createStepGuard(now: () => number = () => performance.now()): StepGuard {
  let unanswered = false;
  let lastMs = -Infinity;
  return {
    send(command, send) {
      if (!STEP_COMMANDS.has(command.type)) return send();
      const at = now();
      if (unanswered || at - lastMs < STEP_PAUSE_MS) return null;
      unanswered = true;
      lastMs = at;
      return send().finally(() => {
        unanswered = false;
      });
    },
  };
}
