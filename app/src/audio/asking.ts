import { useState } from "react";
import type { GameView } from "../types";

/**
 * Which asking of the question on screen `view` is in, given the one
 * before (`last`): the step number (`GameView.stepNo`) at which its step
 * was last `ready`. Restarting a question enters its slide again, so it
 * comes with a new number although the slide stays. Null until a
 * question is seen ready, such as in a show resumed while its clock ran.
 */
export function askingOf(last: number | null, view: GameView | null): number | null {
  const ready = view?.phase.phase === "question" && view.phase.step === "ready";
  return ready ? view.stepNo : last;
}

/**
 * The asking of the question on screen (`askingOf`), for what starts
 * afresh each time a question is asked: its sound and its reading.
 */
export function useAsking(view: GameView | null): number | null {
  const [last, setLast] = useState<number | null>(null);
  const asking = askingOf(last, view);
  if (asking !== last) setLast(asking);
  return asking;
}
