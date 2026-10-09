import type { StringKey } from "./i18n";
import type { Quiz, Slide, SlideType } from "./types";

/** How each kind of slide is named in the UI. */
export const SLIDE_TYPE_LABEL: Record<SlideType, StringKey> = {
  billboard: "typeBillboard",
  question: "typeQuestion",
  endOfRound: "typeEndOfRound",
};

/** The line a slide is listed by: the question text, or the title. */
export function slideTitle(slide: Slide): string {
  return slide.type === "question" ? slide.text : slide.title;
}

/** For each slide its question number (1-based), or null if it is not a question. */
export function questionNumbers(quiz: Quiz): (number | null)[] {
  let n = 0;
  return quiz.slides.map((slide) => (slide.type === "question" ? ++n : null));
}

/** Position of a question among the questions, e.g. 3 of 12. */
export interface QuestionPosition {
  n: number;
  total: number;
}

/** Position of a slide among the questions; null for other slides. */
export function questionPosition(quiz: Quiz | null, slideIndex: number): QuestionPosition | null {
  if (!quiz) return null;
  const numbers = questionNumbers(quiz);
  const n = numbers[slideIndex];
  return n == null ? null : { n, total: numbers.filter((x) => x !== null).length };
}
