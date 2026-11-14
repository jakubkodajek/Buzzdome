// Questions to and from spreadsheets (spec QZ-8): what the editor adds to
// the backend's import and export.

import { DEFAULT_POINTS, DEFAULT_TIME_LIMIT_S, SHEET_WORDS } from "../bindings/Constants";
import type { Lang, StringKey, Translate } from "../i18n";
import type { ListSeparator, QuestionSlide, Quiz, Slide } from "../types";
import { fullyCorrect, newSlide, setBodyText, setHeadline, setNotes } from "./slides";

/**
 * The character between a CSV file's cells. Excel splits a CSV file it
 * opens at the system's list separator: a semicolon where the decimal mark
 * is a comma, as in Czech, else a comma. The quizmaster's language stands
 * in for the system's.
 */
export function listSeparator(lang: Lang): ListSeparator {
  return lang === "cs" ? "semicolon" : "comma";
}

/** The answer time the template's first question shows, not the default, so its cell is filled. */
const TEMPLATE_TIME_S = 30;
/** The points the template's second question shows, not the default, so its cell is filled. */
const TEMPLATE_POINTS = 20;

/**
 * `quiz` with sample slides in the quizmaster's language, whose export is
 * the import template: they use only the basic columns, so the template
 * stays short.
 */
export function templateQuiz(t: Translate, quiz: Quiz): Quiz {
  const question: QuestionSlide = {
    ...(newSlide("question", "t-question") as QuestionSlide),
    text: t("templateQuestion"),
    answers: [t("templateAnswer1"), t("templateAnswer2"), t("templateAnswer3"), t("templateAnswer4")],
    correct: [fullyCorrect(0)],
    timeLimitS: TEMPLATE_TIME_S,
  };
  const open: QuestionSlide = {
    ...question,
    id: "t-open",
    text: t("templateOpen"),
    mode: "fastestFinger",
    answers: [],
    correct: [],
    timeLimitS: DEFAULT_TIME_LIMIT_S,
    points: TEMPLATE_POINTS,
  };
  const slides: Slide[] = [
    setBodyText(setHeadline(newSlide("billboard", "t-round"), t("templateRound")), t("templateRoundText")),
    setNotes(question, t("templateQuestionNotes", { seconds: TEMPLATE_TIME_S, defaultSeconds: DEFAULT_TIME_LIMIT_S })),
    setNotes(open, t("templateOpenNotes", { points: TEMPLATE_POINTS, defaultPoints: DEFAULT_POINTS })),
    setHeadline(newSlide("explanation", "t-explanation"), t("templateExplanation")),
    setHeadline(newSlide("endOfRound", "t-end"), t("templateEnd")),
  ];
  return { ...quiz, slides };
}

/** A small sheet as the import reads it: its headings and rows of cells. */
export interface ExampleSheet {
  headings: string[];
  rows: string[][];
}

/**
 * The example sheet File › Import shows: a round's section, an
 * everyone-answers question with its own answer time, one answered aloud
 * for more points, and the end of the round,
 * under the headings and types the import reads in `lang` (`SHEET_WORDS`,
 * from Rust), with the template's sample texts.
 */
export function exampleSheet(t: Translate, lang: Lang): ExampleSheet {
  const { columns, types } = SHEET_WORDS[lang];
  const answers = [t("templateAnswer1"), t("templateAnswer2"), t("templateAnswer3"), t("templateAnswer4")];
  const row = (type: string, text: string, cells: string[] = [], time = "", points = "") => [type, text, ...[0, 1, 2, 3].map((i) => cells[i] ?? ""), time, points];
  return {
    headings: [columns.type, columns.question, columns.answer1, columns.answer2, columns.answer3, columns.answer4, columns.answerTime, columns.points],
    rows: [
      row(types.section, t("templateRound")),
      row(types.question, t("templateQuestion"), [`[${answers[0]}]`, ...answers.slice(1)], String(TEMPLATE_TIME_S)),
      row(types.question, t("templateOpen"), [], "", String(TEMPLATE_POINTS)),
      row(types.endOfRound, t("templateEnd")),
    ],
  };
}

type SheetType = keyof (typeof SHEET_WORDS)["cs"]["types"];

/**
 * What each value of the Type column makes, in the order File › Import
 * lists them; a type Rust adds to `SHEET_WORDS` is a type error here.
 */
const TYPE_HINTS: Record<SheetType, StringKey> = {
  question: "sheetTypeQuestion",
  fastestFinger: "sheetTypeFastestFinger",
  section: "sheetTypeSection",
  explanation: "sheetTypeExplanation",
  endOfRound: "sheetTypeEndOfRound",
  results: "sheetTypeResults",
};

/** Every slide type the Type column can name in `lang` (`SHEET_WORDS`, from Rust), with what it makes. */
export function sheetTypes(t: Translate, lang: Lang): { type: string; hint: string }[] {
  const { types } = SHEET_WORDS[lang];
  return (Object.entries(TYPE_HINTS) as [SheetType, StringKey][]).map(([key, hint]) => ({ type: types[key], hint: t(hint) }));
}
