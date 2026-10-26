import { FINAL_LAST_STEP } from "../bindings/Constants";
import { shareOf } from "../quiz";
import type { Controls, GameView, Phase, Slide } from "../types";

/** A preview is not a show: no command applies. */
export const NO_CONTROLS: Controls = { next: false, judge: false, pauseClock: false, resumeClock: false, closeQuestion: false, playVideo: false, goTo: false, restartQuestion: false };

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
    nextSlide: null,
    previousSlide: null,
    clock: null,
    players: [],
    teams: [],
    enabledCount: 0,
    answeredCount: 0,
    answerCounts: null,
    answerShares: answerShares(slide),
    results: null,
    buzzed: null,
    leaderboardVisible: false,
    controls: NO_CONTROLS,
    onHold: false,
    videoPlays: 0,
    stepNo: 0,
    autoStep: null,
  };
}

/** A slide's answers' shares as written, for a view the core did not make (a show's view has the core's). */
export function answerShares(slide: Slide | null): (number | null)[] {
  return slide?.type === "question" ? slide.answers.map((_, i) => shareOf(slide, i)) : [];
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
    case "final":
      // The podium with its three places shown.
      return { phase: "final", step: FINAL_LAST_STEP - 1 };
  }
}
