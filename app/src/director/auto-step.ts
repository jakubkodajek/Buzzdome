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
 * The bar of `view`'s coming step: it runs over the whole wait the show
 * gives it (`AutoStepView.totalMs`), never less than what is left, so a bar
 * that comes in during the wait starts part way down.
 */
export function autoStepBar(view: GameView): AutoStepBar | null {
  const step = view.autoStep;
  if (!step) return null;
  const totalMs = Math.max(step.totalMs, step.inMs);
  return { key: `${step.action}:${view.stepNo}`, totalMs, elapsedMs: totalMs - step.inMs };
}
