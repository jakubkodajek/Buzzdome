// The show's results (spec SF-11) as the Director's Results tab and the
// results export lay them out. The results themselves come from the core
// (`Game::show_results`); this only words and orders them.

import { seconds } from "../format";
import { playerName, type Translate } from "../i18n";
import { slideContexts, slideTitle, type SlideContext } from "../quiz";
import type { AskedQuestion, BuzzerId, ListSeparator, Mark, Quiz, ShowResults, Slide } from "../types";

/** How the results name each question asked, by its slide: its number among the quiz's questions and its text. */
export function questionLabels(quiz: Quiz): (slide: number) => string {
  const contexts = slideContexts(quiz);
  return (slide) => {
    const asked: Slide | undefined = quiz.slides[slide];
    const context: SlideContext | undefined = contexts[slide];
    const n = context?.position?.n ?? slide + 1;
    return asked ? `${n}. ${slideTitle(asked)}` : String(n);
  };
}

/** What a question gave the players it scored, the fastest answer first and no answer last. */
export function marksByTime(question: AskedQuestion): [BuzzerId, Mark][] {
  const time = ([, mark]: [BuzzerId, Mark]) => mark.answer?.elapsedMs ?? Number.POSITIVE_INFINITY;
  return [...question.marks].sort((a, b) => time(a) - time(b));
}

/** The rounds the results add up separately, from 0: none when the quiz has only one. */
export function resultRounds(results: ShowResults): number[] {
  return results.rounds > 1 ? Array.from({ length: results.rounds }, (_, round) => round) : [];
}

/**
 * The results as a table for a spreadsheet, a row for each player in the
 * ranking's order: their place, what each question asked and each round
 * gave them, the points added by hand, their score and their answering
 * time in seconds. The decimal mark goes with the file's `separator`, as a
 * spreadsheet program set for that list separator reads numbers.
 */
export function resultsTable(t: Translate, quiz: Quiz, results: ShowResults, separator: ListSeparator): string[][] {
  const label = questionLabels(quiz);
  const rounds = resultRounds(results);
  const decimal = (ms: number) => (separator === "semicolon" ? seconds(ms, 2).replace(".", ",") : seconds(ms, 2));
  const header = [
    t("colPlace"),
    t("colBuzzer"),
    t("name"),
    ...results.questions.map((question) => label(question.slide)),
    ...rounds.map((round) => t("colRound", { n: round + 1 })),
    t("colAdjusted"),
    t("score"),
    t("colTotalTimeSeconds"),
  ];
  const rows = results.players.map((player) => [
    player.rank === null ? "" : String(player.rank),
    String(player.buzzer),
    playerName(t, player),
    ...results.questions.map((question) => {
      const mark = question.marks.find(([buzzer]) => buzzer === player.buzzer);
      return mark ? String(mark[1].points) : "";
    }),
    ...rounds.map((round) => String(player.rounds[round] ?? 0)),
    String(player.adjusted),
    String(player.score),
    decimal(player.totalAnswerMs),
  ]);
  return [header, ...rows];
}
