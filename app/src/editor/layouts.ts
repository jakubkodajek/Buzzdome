// The slide layouts the INSERT gallery offers. A layout is a starting point:
// what it sets can all be changed afterwards.

import type { StringKey } from "../i18n";
import type { AnswerMode, Slide, SlideType } from "../types";
import { newSlide } from "./slides";

export interface SlideLayout {
  type: SlideType;
  /** Answers of a question (0: answered aloud, fastest finger). */
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
];

/** A new slide laid out as `layout` (its media still to be chosen). */
export function slideFromLayout(layout: SlideLayout, id: string): Slide {
  const slide = newSlide(layout.type, id);
  if (slide.type !== "question" || layout.answers === undefined) return slide;
  const mode: AnswerMode = layout.answers === 0 ? "fastestFinger" : "everyone";
  return { ...slide, mode, answers: Array<string>(layout.answers).fill(""), correct: layout.answers === 0 ? null : 0 };
}
