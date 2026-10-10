// Questions to and from spreadsheets (spec QZ-8): what the editor adds to
// the backend's import and export.

import type { Lang, Translate } from "../i18n";
import type { ListSeparator, QuestionSlide, Quiz, Slide } from "../types";
import { fullyCorrect, newSlide, setBodyText, setHeadline, setNotes } from "./slides";

/** File types questions are imported from: workbooks and CSV files. */
export const SHEET_EXTENSIONS = ["xlsx", "xlsm", "xls", "ods", "csv"];
export const CSV_EXTENSION = "csv";

/**
 * The character between a CSV file's cells. Excel splits a CSV file it
 * opens at the system's list separator: a semicolon where the decimal mark
 * is a comma, as in Czech, else a comma. The quizmaster's language stands
 * in for the system's.
 */
export function listSeparator(lang: Lang): ListSeparator {
  return lang === "cs" ? "semicolon" : "comma";
}

/**
 * `quiz` with sample slides in the quizmaster's language, whose export is
 * the import template. The sample questions take on the quiz's first
 * category, if it has one, so its CAT column shows how one is named.
 */
export function templateQuiz(t: Translate, quiz: Quiz): Quiz {
  const question: QuestionSlide = {
    ...(newSlide("question", "t-question") as QuestionSlide),
    category: quiz.categories?.[0]?.id,
    text: t("templateQuestion"),
    answers: [t("templateAnswer1"), t("templateAnswer2"), t("templateAnswer3"), t("templateAnswer4")],
    correct: [fullyCorrect(0)],
  };
  const open: QuestionSlide = { ...question, id: "t-open", text: t("templateOpen"), mode: "fastestFinger", answers: [], correct: [] };
  const slides: Slide[] = [
    setBodyText(setHeadline(newSlide("billboard", "t-round"), t("templateRound")), t("templateRoundText")),
    setNotes(question, t("templateQuestionNotes")),
    setNotes(open, t("templateOpenNotes")),
    setHeadline(newSlide("explanation", "t-explanation"), t("templateExplanation")),
    setHeadline(newSlide("endOfRound", "t-end"), t("templateEnd")),
  ];
  return { ...quiz, slides };
}
