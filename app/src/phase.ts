import { playerName, type Translate } from "./i18n";
import type { GameView } from "./types";

/** Short status of the show for the Director header. */
export function phaseLabel(t: Translate, view: GameView): string {
  const phase = view.phase;
  switch (phase.phase) {
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
          return t("buzzed", { name: player ? playerName(t, player) : String(phase.buzzer) });
        }
        case "closed":
          return t("closed");
        case "revealed":
          return t("revealed");
      }
  }
}
