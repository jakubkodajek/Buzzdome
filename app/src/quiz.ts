import { FULL_SHARE, SLIDE_PLAYS } from "./bindings/Constants";
import type { StringKey } from "./i18n";
import type { Plays, QuestionSlide, Quiz, Slide, SlideType, TitledSlide } from "./types";

/** How each kind of slide is named in the UI. */
export const SLIDE_TYPE_LABEL: Record<SlideType, StringKey> = {
  billboard: "typeBillboard",
  question: "typeQuestion",
  explanation: "typeExplanation",
  endOfRound: "typeEndOfRound",
  final: "typeFinal",
};

/** The line a slide is listed by: the question or explanation text, or the title. */
export function slideTitle(slide: Slide): string {
  return hasTitle(slide) ? slide.title : slide.text;
}

export function hasTitle(slide: Slide): slide is TitledSlide {
  return slide.type === "billboard" || slide.type === "endOfRound" || slide.type === "final";
}

/**
 * What a slide of the type can play (`SlideType::plays` in quiz-core): a
 * picture or video (a ranking shows the ranking instead), and a clock and
 * a reveal (only a question).
 */
export function plays(type: SlideType): Plays {
  return SLIDE_PLAYS[type];
}

/**
 * The share of the points answer `index` gives, in percent (`FULL_SHARE` for
 * a fully right one); null for a wrong answer. A fastest-finger question
 * gives all of them (`Question::share_of` in quiz-core).
 */
export function shareOf(question: Pick<QuestionSlide, "correct" | "mode">, index: number): number | null {
  const share = question.correct.find((c) => c.answer === index)?.share ?? null;
  return share !== null && question.mode === "fastestFinger" ? FULL_SHARE : share;
}

export function isCorrect(question: Pick<QuestionSlide, "correct">, index: number): boolean {
  return question.correct.some((c) => c.answer === index);
}

/** Position of a question among the questions, e.g. 3 of 12. */
export interface QuestionPosition {
  n: number;
  total: number;
}

/** A question as an explanation slide shows it. */
export interface ExplainedQuestion {
  question: QuestionSlide;
  position: QuestionPosition;
}

/** What a slide shows from the rest of the quiz. */
export interface SlideContext {
  /** Its place among the questions; null for other slides. */
  position: QuestionPosition | null;
  /**
   * For an explanation slide, the question it explains: the nearest one
   * before it in the same round. Null when there is none.
   */
  explains: ExplainedQuestion | null;
}

export const NO_CONTEXT: SlideContext = { position: null, explains: null };

export function questionCount(quiz: Quiz): number {
  return quiz.slides.filter((slide) => slide.type === "question").length;
}

/** The context of each slide of `quiz`, in order. */
export function slideContexts(quiz: Quiz): SlideContext[] {
  const total = questionCount(quiz);
  let n = 0;
  let lastQuestion: ExplainedQuestion | null = null;
  return quiz.slides.map((slide) => {
    switch (slide.type) {
      case "question": {
        const position = { n: ++n, total };
        lastQuestion = { question: slide, position };
        return { position, explains: null };
      }
      case "explanation":
        return { position: null, explains: lastQuestion };
      case "endOfRound":
      case "final":
        lastQuestion = null;
        return NO_CONTEXT;
      case "billboard":
        return NO_CONTEXT;
    }
  });
}

/** The context of the slide at `index`; none without a quiz. */
export function slideContext(quiz: Quiz | null, index: number): SlideContext {
  return (quiz && slideContexts(quiz)[index]) ?? NO_CONTEXT;
}

/** Whether a slide shows the same in both contexts. */
export function sameContext(a: SlideContext, b: SlideContext): boolean {
  const samePosition = (x: QuestionPosition | null, y: QuestionPosition | null) => x?.n === y?.n && x?.total === y?.total;
  return (
    samePosition(a.position, b.position) &&
    a.explains?.question === b.explains?.question &&
    samePosition(a.explains?.position ?? null, b.explains?.position ?? null)
  );
}
