import { playerName, type Translate } from "../i18n";
import type { GameView } from "../types";

/** Answers of the current question as they arrive, fastest first. */
export function ResponsesTab({ t, view }: { t: Translate; view: GameView }) {
  const responses = view.players
    .filter((p) => p.answer !== null)
    .sort((a, b) => (a.answerMs ?? 0) - (b.answerMs ?? 0));
  const question = view.slide?.type === "question" ? view.slide : null;

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
          {responses.map((p) => {
            const index = { a: 0, b: 1, c: 2, d: 3, big: -1 }[p.answer!];
            const correct = question?.correct === index;
            return (
              <tr key={p.buzzer}>
                <td className="num">{p.buzzer}</td>
                <td>{playerName(t, p)}</td>
                <td className={correct ? "good-text strong" : ""}>{p.answer!.toUpperCase()}</td>
                <td className="num">{((p.answerMs ?? 0) / 1000).toFixed(2)} s</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
