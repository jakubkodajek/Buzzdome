import type { GameView, Phase, Slide } from "../types";

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
