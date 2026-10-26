import { buttonLabel } from "../answers";
import { useApp } from "../app-context";
import { seconds } from "../format";
import { playerName } from "../i18n";
import type { AnswerResults, GameView } from "../types";

/**
 * Answers of the current question as they arrive, fastest first, and once
 * an everyone-answers question closes, how the players it counted did.
 */
export function ResponsesTab({ view }: { view: GameView }) {
  const { t } = useApp();
  const responses = view.players
    .flatMap((p) => (p.answer === null ? [] : [{ player: p, answer: p.answer }]))
    .sort((a, b) => (a.player.answerMs ?? 0) - (b.player.answerMs ?? 0));
  const none = responses.length === 0 && view.buzzed === null;

  return (
    <div className="responses-tab">
      {view.results && <ResultStrip results={view.results} />}
      {none ? (
        <p className="empty">{t("noResponses")}</p>
      ) : (
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
      )}
    </div>
  );
}

/** How many of the players a closed question counted answered right, wrong or not at all, as a strip and in numbers. */
function ResultStrip({ results }: { results: AnswerResults }) {
  const { t } = useApp();
  const parts = [
    { kind: "right", label: t("right"), n: results.right },
    { kind: "wrong", label: t("wrong"), n: results.wrong },
    { kind: "none", label: t("noAnswer"), n: results.none },
  ];
  return (
    <div className="result-strip">
      <div className="result-strip-bar">
        {parts.map(({ kind, n }) => n > 0 && <span key={kind} className={kind} style={{ flexGrow: n }} />)}
      </div>
      {parts.map(({ kind, label, n }) => (
        <span key={kind} className={`result-strip-count ${kind}`}>
          {label} <strong>{n}</strong>
        </span>
      ))}
    </div>
  );
}
