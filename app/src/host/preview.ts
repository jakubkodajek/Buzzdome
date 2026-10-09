import type { Controls, GameView, Phase, Slide } from "../types";

/** A preview is not a show: no command applies. */
export const NO_CONTROLS: Controls = { next: false, judge: false, pauseClock: false, resumeClock: false, closeQuestion: false };

/**
 * The game state a slide is shown in before anyone plays: a question ready
 * to start, with nobody registered. The editor draws slides with it.
 */
export function previewView(slide: Slide, slideIndex: number, slideCount: number, quizTitle: string): GameView {
  return {
    quizTitle,
    slideIndex,
    slideCount,
    phase: previewPhase(slide),
    slide,
    nextSlideTitle: null,
    clock: null,
    players: [],
    enabledCount: 0,
    answeredCount: 0,
    answerCounts: null,
    results: null,
    buzzed: null,
    leaderboardVisible: false,
    controls: NO_CONTROLS,
  };
}

function previewPhase(slide: Slide): Phase {
  switch (slide.type) {
    case "question":
      return { phase: "question", step: "ready" };
    case "billboard":
      return { phase: "billboard" };
    case "explanation":
      return { phase: "explanation" };
    case "endOfRound":
      return { phase: "endOfRound" };
  }
}
