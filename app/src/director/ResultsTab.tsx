import { ANSWER_LETTERS, buttonLabel } from "../answers";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { seconds, signed } from "../format";
import { useShowResults } from "../hooks";
import { playerName } from "../i18n";
import { rightAnswers } from "../quiz";
import type { BuzzerId, GameView, PlayerResult, Quiz, ShowResults, Slide } from "../types";
import { listSeparator } from "../editor/sheet";
import { marksByTime, questionLabels, resultRounds, resultsTable } from "./results";

/**
 * The show's results so far (spec SF-11): the ranking with what each round
 * gave each player, and every question asked with what it gave whom. They
 * export to a spreadsheet and print, or save as a PDF, from here.
 */
export function ResultsTab({ view, quiz }: { view: GameView; quiz: Quiz | null }) {
  const { t, lang, notify } = useApp();
  const run = useRun();
  const results = useShowResults(view);
  if (!results || !quiz) return null;

  const exportCsv = () => {
    const picker = { filter: t("csvFile"), fileName: t("resultsFileName", { title: quiz.title || t("untitledQuiz") }) };
    const rows = resultsTable(t, quiz, results, listSeparator(lang));
    run(api.exportResults(rows, listSeparator(lang), picker).then((saved) => saved && notify(t("resultsSaved"))));
  };

  return (
    <div className="results-tab">
      <div className="toolbar">
        <button onClick={exportCsv}>{t("resultsExport")}</button>
        <button title={t("resultsPrintHint")} onClick={() => run(api.printWindow())}>
          {t("resultsPrint")}
        </button>
      </div>
      <div className="results-body">
        <h1 className="results-title">{quiz.title || t("untitledQuiz")}</h1>
        <h2>{t("resultsRanking")}</h2>
        <Ranking results={results} />
        <h2>{t("resultsQuestions")}</h2>
        <Questions results={results} quiz={quiz} />
      </div>
    </div>
  );
}

function Ranking({ results }: { results: ShowResults }) {
  const { t } = useApp();
  const rounds = resultRounds(results);
  return (
    <table className="grid">
      <thead>
        <tr>
          <th className="num">{t("colPlace")}</th>
          <th>{t("name")}</th>
          <th className="num">{t("colBuzzer")}</th>
          {rounds.map((round) => (
            <th key={round} className="num">
              {t("colRound", { n: round + 1 })}
            </th>
          ))}
          <th className="num" title={t("colAdjustedHint")}>
            {t("colAdjusted")}
          </th>
          <th className="num">{t("score")}</th>
          <th className="num">{t("colTotalTime")}</th>
        </tr>
      </thead>
      <tbody>
        {results.players.map((player) => (
          <tr key={player.buzzer} className={player.rank === null ? "disabled" : ""}>
            <td className="num">{player.rank ?? ""}</td>
            <td>{playerName(t, player)}</td>
            <td className="num">{player.buzzer}</td>
            {rounds.map((round) => (
              <td key={round} className="num">
                {player.rounds[round] ?? 0}
              </td>
            ))}
            <td className="num">{player.adjusted === 0 ? "" : signed(player.adjusted)}</td>
            <td className="num strong">{player.score}</td>
            <td className="num">{seconds(player.totalAnswerMs, 2)} s</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Questions({ results, quiz }: { results: ShowResults; quiz: Quiz }) {
  const { t } = useApp();
  if (results.questions.length === 0) return <p className="muted">{t("noQuestionsAsked")}</p>;
  const label = questionLabels(quiz);
  const players = new Map<BuzzerId, PlayerResult>(results.players.map((player) => [player.buzzer, player]));
  const name = (buzzer: BuzzerId) => playerName(t, players.get(buzzer) ?? { name: "", buzzer });
  return (
    <>
      {results.questions.map((question, asked) => {
        const slide: Slide | undefined = quiz.slides[question.slide];
        const right = slide?.type === "question" ? rightAnswers(slide).map(({ answer }) => ANSWER_LETTERS[answer]) : [];
        const marks = marksByTime(question);
        return (
          // A question asked again on a revisit (shows before rules version 6) is listed again.
          <section key={asked} className="results-question">
            <h3>
              {label(question.slide)}
              {right.length > 0 && <span className="muted"> · {t("rightAnswersAre", { answers: right.join(", ") })}</span>}
            </h3>
            {marks.length === 0 ? (
              <p className="muted">{t("nobodyAnswered")}</p>
            ) : (
              <table className="grid">
                <thead>
                  <tr>
                    <th>{t("name")}</th>
                    <th className="num">{t("colBuzzer")}</th>
                    <th>{t("colResponse")}</th>
                    <th className="num">{t("colTime")}</th>
                    <th className="num">{t("score")}</th>
                  </tr>
                </thead>
                <tbody>
                  {marks.map(([buzzer, mark]) => (
                    <tr key={buzzer}>
                      <td>{name(buzzer)}</td>
                      <td className="num">{buzzer}</td>
                      <td className={mark.right ? "good-text strong" : ""}>{mark.answer ? buttonLabel(mark.answer.button) : t("noAnswer")}</td>
                      <td className="num">{mark.answer ? `${seconds(mark.answer.elapsedMs, 2)} s` : ""}</td>
                      <td className="num">
                        {signed(mark.points)}
                        {mark.joker && <span className="results-joker"> {t("jokerPlay")}</span>}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        );
      })}
    </>
  );
}
