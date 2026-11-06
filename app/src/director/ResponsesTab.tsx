import { buttonLabel } from "../answers";
import { useApp } from "../app-context";
import { seconds } from "../format";
import { playerName, type StringKey } from "../i18n";
import type { AnswerResults, BuzzState, GameView } from "../types";

/** How a buzz stands, in words. */
const BUZZ_STATE = { wrong: "wrong", answering: "buzzAnswering", waiting: "buzzWaiting", right: "right" } as const satisfies Record<BuzzState, StringKey>;

/**
 * Answers of the current question as they arrive, fastest first, and once
 * an everyone-answers question closes, how the players it counted did. In
 * fastest finger, every buzz in the order it came, with how it stands.
 */
export function ResponsesTab({ view }: { view: GameView }) {
  const { t } = useApp();
  const responses = view.players
    .flatMap((p) => (p.answer === null ? [] : [{ player: p, answer: p.answer }]))
    .sort((a, b) => (a.player.answerMs ?? 0) - (b.player.answerMs ?? 0));
  const none = responses.length === 0 && view.buzzes.length === 0;

  return (
    <div className="responses-tab">
      {view.results && <ResultStrip results={view.results} />}
      {view.buzzes.length > 0 && (
        <div className="table-wrap">
          <table className="grid">
            <thead>
              <tr>
                <th>{t("colTurn")}</th>
                <th>{t("colBuzzer")}</th>
                <th>{t("name")}</th>
                <th>{t("colResponse")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {view.buzzes.map((buzz, i) => {
                const player = view.players.find((p) => p.buzzer === buzz.buzzer);
                const tone = buzz.state === "right" ? "good-text strong" : buzz.state === "answering" ? "strong" : buzz.state === "wrong" ? "muted" : "";
                return (
                  <tr key={buzz.buzzer} className={buzz.state === "answering" ? "current" : ""}>
                    <td className="num">{i + 1}</td>
                    <td className="num">{buzz.buzzer}</td>
                    <td>{player ? playerName(t, player) : buzz.buzzer}</td>
                    <td>{buttonLabel(buzz.button)}</td>
                    <td className={tone}>{t(BUZZ_STATE[buzz.state])}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      {none && <p className="empty">{t("noResponses")}</p>}
      {responses.length > 0 && (
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
