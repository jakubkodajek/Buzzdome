// The answer chart (quiz setting answerChart, spec QZ-11): once an
// everyone-answers question's answer is shown, a bar on each answer for the
// share of the players the question counted who gave it.

import type { GameView } from "../types";

/** Each answer's share of the players the current question counted, from 0 to 1; none while there is no chart to draw. */
export function chartShares(view: GameView): number[] | null {
  const revealed = view.phase.phase === "question" && view.phase.step === "revealed";
  if (!revealed || !view.settings.answerChart || !view.answerCounts || !view.results) return null;
  const counted = view.results.right + view.results.wrong + view.results.none;
  return counted > 0 ? view.answerCounts.map((count) => count / counted) : null;
}
