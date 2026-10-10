import { buttonLabel } from "../answers";
import { useApp } from "../app-context";
import { seconds } from "../format";
import { playerName } from "../i18n";
import type { GameView } from "../types";

/** Answers of the current question as they arrive, fastest first. */
export function ResponsesTab({ view }: { view: GameView }) {
  const { t } = useApp();
  const responses = view.players
    .flatMap((p) => (p.answer === null ? [] : [{ player: p, answer: p.answer }]))
    .sort((a, b) => (a.player.answerMs ?? 0) - (b.player.answerMs ?? 0));

  if (responses.length === 0 && view.buzzed === null) return <p className="empty">{t("noResponses")}</p>;
  return (
    <div className="table-wrap">
      <table className="grid">
        <thead>
          <tr>
            <th>{t("colBuzzer")}</th>
            <th>{t("name")}</th>
            <th>{t("colResponse")}</th>
            <th>{t("colTime")}</th>
          </tr>
        </thead>
        <tbody>
          {responses.map(({ player, answer }) => (
            <tr key={player.buzzer}>
              <td className="num">{player.buzzer}</td>
              <td>{playerName(t, player)}</td>
              <td className={player.answerCorrect ? "good-text strong" : ""}>{buttonLabel(answer)}</td>
              <td className="num">{seconds(player.answerMs ?? 0, 2)} s</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
