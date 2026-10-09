import type { Quiz, Slide } from "./types";

/** The line a slide is listed by: the question text, or the title. */
export function slideTitle(slide: Slide): string {
  return slide.type === "question" ? slide.text : slide.title;
}

/** For each slide its question number (1-based), or null if it is not a question. */
export function questionNumbers(quiz: Quiz): (number | null)[] {
  let n = 0;
  return quiz.slides.map((slide) => (slide.type === "question" ? ++n : null));
}

/** Position of a slide among the questions, e.g. 3 of 12; null for other slides. */
export function questionPosition(quiz: Quiz | null, slideIndex: number): { n: number; total: number } | null {
  if (!quiz) return null;
  const numbers = questionNumbers(quiz);
  const n = numbers[slideIndex];
  return n == null ? null : { n, total: numbers.filter((x) => x !== null).length };
}
