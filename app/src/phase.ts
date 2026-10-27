import { ANSWER_LETTERS, answerIndex } from "./answers";
import { playerName, type StringKey, type Translate } from "./i18n";
import type { Cue, GameView, Phase } from "./types";

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
          const player = view.players.find((p) => p.buzzer === phase.buzzer);
          const name = player ? playerName(t, player) : String(phase.buzzer);
          // Buzzed in with an answer button: that is the answer to judge.
          const answer = answerIndex(phase.button);
          return answer === null ? t("buzzed", { name }) : t("buzzedAnswer", { name, letter: ANSWER_LETTERS[answer] });
        }
        case "closed":
          return t("closed");
        case "revealed":
          return t("revealed");
      }
  }
}
