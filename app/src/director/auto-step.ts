import type { GameView } from "../types";

/** How the Director's bar for a step the show takes by itself runs. */
export interface AutoStepBar {
  /** One wait: the bar keeps running while views come during it. */
  key: string;
  /** The whole wait, the bar's run from full to empty. */
  totalMs: number;
  /** How far into the wait the show already is, where the bar starts. */
  elapsedMs: number;
}

/**
 * The bar of `view`'s coming step: the whole wait is the quiz's delay for
 * it (`QuizSettings`: after everyone answered, or after the reveal), and
 * never less than what is left, so a bar that comes in during the wait
 * starts part way down.
 */
export function autoStepBar(view: GameView): AutoStepBar | null {
  const step = view.autoStep;
  if (!step) return null;
  const revealed = view.phase.phase === "question" && view.phase.step === "revealed";
  const delayS = revealed ? view.settings.nextAfterRevealDelayS : view.settings.allAnsweredDelayS;
  const totalMs = Math.max(delayS * 1000, step.inMs);
  return { key: `${step.action}:${view.stepNo}`, totalMs, elapsedMs: totalMs - step.inMs };
}
