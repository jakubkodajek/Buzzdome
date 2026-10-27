import { DEFAULT_QUIZ_SETTINGS, FINAL_LAST_STEP } from "../bindings/Constants";
import { shareOf } from "../quiz";
import type { Controls, GameView, Phase, Slide, Welcome } from "../types";

/** A preview is not a show: no command applies. */
export const NO_CONTROLS: Controls = { next: false, judge: false, pauseClock: false, resumeClock: false, closeQuestion: false, playVideo: false, goTo: false, restartQuestion: false, setJoker: false };

/**
 * The game state a slide is shown in before anyone plays: a question ready
 * to start, with nobody registered. The editor draws slides with it.
 */
export function previewView(slide: Slide, slideIndex: number, slideCount: number, quizTitle: string): GameView {
  return { ...nobodyPlays(quizTitle, slideCount), slideIndex, phase: previewPhase(slide), slide, answerShares: answerShares(slide) };
}

/** The welcome screen the show opens on, before anyone registered, for the editor to draw. */
export function welcomePreviewView(welcome: Welcome, quizTitle: string, slideCount: number): GameView {
  return { ...nobodyPlays(quizTitle, slideCount), slideIndex: 0, phase: { phase: "welcome" }, slide: null, answerShares: [], welcome };
}

/** A show before anyone plays, without the slide on screen. */
function nobodyPlays(quizTitle: string, slideCount: number): Omit<GameView, "slideIndex" | "phase" | "slide" | "answerShares"> {
  return {
    quizTitle,
    slideCount,
    nextSlide: null,
    previousSlide: null,
    clock: null,
    players: [],
    teams: [],
    enabledCount: 0,
    answeredCount: 0,
    answerCounts: null,
    results: null,
    buzzed: null,
    leaderboardVisible: false,
    controls: NO_CONTROLS,
    onHold: false,
    videoPlays: 0,
    stepNo: 0,
    autoStep: null,
    settings: DEFAULT_QUIZ_SETTINGS,
    music: null,
    welcome: null,
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
