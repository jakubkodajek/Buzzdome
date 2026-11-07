import { ANSWER_LETTERS, answerIndex } from "./answers";
import { playerName, type StringKey, type Translate } from "./i18n";
import type { Button, BuzzerId, Cue, GameView, Phase } from "./types";

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
        case "buzzed":
          return buzzedLabel(t, view, phase.buzzer, phase.button, false);
        case "closed":
          return t("closed");
        case "revealed":
          return t("revealed");
      }
  }
}

/**
 * Who is being judged now, with the answer button they buzzed with and the
 * buzzer number of a named player, as the Director shows it beside Right
 * and Wrong; null when nobody is.
 */
export function judgedLabel(t: Translate, view: GameView): string | null {
  const phase = view.phase;
  if (phase.phase !== "question" || phase.step !== "buzzed") return null;
  return buzzedLabel(t, view, phase.buzzer, phase.button, true);
}

/** "Buzzed: name", the answer if any, (`withBuzzer`) the buzzer of a named player, and the buzzes waiting. */
function buzzedLabel(t: Translate, view: GameView, buzzer: BuzzerId, button: Button, withBuzzer: boolean): string {
  const player = view.players.find((p) => p.buzzer === buzzer);
  const name = player ? playerName(t, player) : String(buzzer);
  // Buzzed in with an answer button: that is the answer to judge.
  const answer = answerIndex(button);
  const parts = [answer === null ? t("buzzed", { name }) : t("buzzedAnswer", { name, letter: ANSWER_LETTERS[answer] })];
  if (withBuzzer && player?.name) parts.push(t("buzzer", { n: buzzer }));
  const waiting = view.buzzes.filter((buzz) => buzz.state === "waiting").length;
  if (waiting > 0) parts.push(t("buzzesWaiting", { n: waiting }));
  return parts.join(" · ");
}

/**
 * The slide on the projector, or null before the slides (the welcome screen
 * and registration) and in the ranking after the last slide.
 */
export function currentSlide(view: GameView): number | null {
  return beforeSlides(view.phase) || view.slideIndex >= view.slideCount ? null : view.slideIndex;
}
