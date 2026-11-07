import { ANSWER_LETTERS, answerIndex } from "./answers";
import { playerName, type StringKey, type Translate } from "./i18n";
import type { Button, BuzzerId, Cue, GameView, NextStep, Phase } from "./types";

/** What each moment the show has a sound and a light flash for is called. */
export const CUE_LABEL: Record<Cue, StringKey> = {
  clockStart: "cueClockStart",
  buzz: "cueBuzz",
  correct: "cueCorrect",
  wrong: "cueWrong",
  timeUp: "cueTimeUp",
  reveal: "cueReveal",
  leaderboard: "cueLeaderboard",
  place: "cuePlace",
  winner: "cueWinner",
};

/** Whether the show is on its welcome screen or in the lobby, before the first slide (`Game::before_slides`). */
export function beforeSlides(phase: Phase): boolean {
  return phase.phase === "welcome" || phase.phase === "lobby";
}

/** Short status of the show for the Director header. */
export function phaseLabel(t: Translate, view: GameView): string {
  const phase = view.phase;
  switch (phase.phase) {
    case "welcome":
      return t("welcomeScreen");
    case "lobby":
      return t("lobby");
    case "billboard":
      return t("billboard");
    case "explanation":
      return t("explanation");
    case "endOfRound":
      return t("endOfRound");
    case "final":
      return t("final");
    case "question":
      switch (phase.step) {
        case "ready":
          return t("ready");
        case "running":
          return view.clock?.running ? t("running") : t("paused");
        case "buzzed": {
          const { who, detail } = buzzedLabel(t, view, phase.buzzer, phase.button, false);
          return detail === null ? who : `${who} · ${detail}`;
        }
        case "closed":
          return t("closed");
        case "revealed":
          return t("revealed");
      }
  }
}

/** Who is being judged now: their name, then what the quizmaster also needs to see. */
export interface Judged {
  /** "Buzzed: name", with the answer they buzzed with, if any. */
  who: string;
  /** The buzzes waiting their turn, then the buzzer of a named player; null when neither applies. */
  detail: string | null;
}

/**
 * Who is being judged now, as the Director shows it beside Right and
 * Wrong; null when nobody is.
 */
export function judgedLabel(t: Translate, view: GameView): Judged | null {
  const phase = view.phase;
  if (phase.phase !== "question" || phase.step !== "buzzed") return null;
  return buzzedLabel(t, view, phase.buzzer, phase.button, true);
}

/**
 * "Buzzed: name" and the answer if any; then the buzzes waiting, which tell
 * that Wrong hands the turn on, and (`withBuzzer`) the buzzer of a named
 * player.
 */
function buzzedLabel(t: Translate, view: GameView, buzzer: BuzzerId, button: Button, withBuzzer: boolean): Judged {
  const player = view.players.find((p) => p.buzzer === buzzer);
  const name = player ? playerName(t, player) : String(buzzer);
  // Buzzed in with an answer button: that is the answer to judge.
  const answer = answerIndex(button);
  const who = answer === null ? t("buzzed", { name }) : t("buzzedAnswer", { name, letter: ANSWER_LETTERS[answer] });
  const details: string[] = [];
  const waiting = view.buzzes.filter((buzz) => buzz.state === "waiting").length;
  if (waiting > 0) details.push(t("buzzesWaiting", { n: waiting }));
  if (withBuzzer && player?.name) details.push(t("buzzer", { n: buzzer }));
  return { who, detail: details.length > 0 ? details.join(" · ") : null };
}

/** How the Next button says what it does in each step (`Controls::next_step`), but the final ranking's, which names its place. */
const NEXT_STEP_LABEL: Record<Exclude<NextStep, { finalStep: unknown }>, StringKey> = {
  registration: "nextRegistration",
  startQuestion: "nextStartQuestion",
  endQuestion: "nextEndQuestion",
  showAnswer: "nextShowAnswer",
  startQuiz: "nextStartQuiz",
  finalRanking: "nextFinalRanking",
  nextSlide: "nextSlide",
};

/** What the Next button does now, in words, as the show says it (`Controls::next_step`). */
export function nextLabel(t: Translate, view: GameView): string {
  const step = view.controls.nextStep;
  if (step === null) return t("next");
  if (typeof step === "string") return t(NEXT_STEP_LABEL[step]);
  const { place } = step.finalStep;
  if (place === null) return t("nextFinalTable");
  return place === 1 ? t("nextFinalWinner") : t("nextFinalPlace", { n: place });
}

/**
 * The slide on the projector, or null before the slides (the welcome screen
 * and registration) and in the ranking after the last slide.
 */
export function currentSlide(view: GameView): number | null {
  return beforeSlides(view.phase) || view.slideIndex >= view.slideCount ? null : view.slideIndex;
}
