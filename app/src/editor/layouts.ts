// The slide layouts the INSERT gallery offers. A layout is a starting point:
// what it sets can all be changed afterwards.

import type { StringKey } from "../i18n";
import type { AnswerMode, Slide, SlideType } from "../types";
import { answerLimits, newSlide } from "./slides";

export interface SlideLayout {
  type: SlideType;
  /**
   * Answers of a question. Everyone answers a question with an answer per
   * button; one with none or more is fastest finger, answered aloud.
   */
  answers?: number;
  /** Asks for a picture or a video once the slide is inserted. */
  media?: "picture" | "video";
}

export interface LayoutGroup {
  title: StringKey;
  layouts: SlideLayout[];
}

const withAndWithoutPicture = (answers: number): SlideLayout[] => [
  { type: "question", answers },
  { type: "question", answers, media: "picture" },
];

/** What a new slide is unless another layout is picked. */
export const DEFAULT_LAYOUT: SlideLayout = { type: "question", answers: 4 };

export const LAYOUT_GROUPS: LayoutGroup[] = [
  {
    title: "layoutsCommon",
    layouts: [
      DEFAULT_LAYOUT,
      { ...DEFAULT_LAYOUT, media: "picture" },
      { type: "question", answers: 0 },
      { type: "question", answers: 0, media: "picture" },
      { type: "billboard" },
      { type: "billboard", media: "picture" },
      { type: "billboard", media: "video" },
      { type: "explanation" },
      { type: "explanation", media: "picture" },
      { type: "endOfRound" },
    ],
  },
  { title: "layouts2", layouts: withAndWithoutPicture(2) },
  { title: "layouts3", layouts: withAndWithoutPicture(3) },
  { title: "layouts5", layouts: withAndWithoutPicture(5) },
  { title: "layouts6", layouts: withAndWithoutPicture(6) },
];

/** The answer mode of a question laid out with `answers` answers. */
export function layoutMode(answers: number): AnswerMode {
  const { fewest, most } = answerLimits("everyone");
  return answers >= fewest && answers <= most ? "everyone" : "fastestFinger";
}

/** A new slide laid out as `layout` (its media still to be chosen). */
export function slideFromLayout(layout: SlideLayout, id: string): Slide {
  const slide = newSlide(layout.type, id);
  if (slide.type !== "question" || layout.answers === undefined) return slide;
  const answers = Array<string>(layout.answers).fill("");
  return { ...slide, mode: layoutMode(layout.answers), answers, correct: layout.answers === 0 ? null : 0 };
}
