// The parts of a slide the editor can select, as marked by `data-element`
// in the slide screens (host/SlideScreen.tsx, slide-media.tsx).

import { hasTitle } from "../quiz";
import type { Slide } from "../types";

export type ElementId = "slide" | "question" | "title" | "text" | "picture" | "video" | "sound" | `answer-${number}`;

/** The parts of a slide that are a media file. */
export type MediaElement = "picture" | "video" | "sound";

export function isMediaElement(element: ElementId): element is MediaElement {
  return element === "picture" || element === "video" || element === "sound";
}

/** The answer an element is, or null. */
export function answerIndex(element: ElementId): number | null {
  const match = /^answer-(\d+)$/.exec(element);
  return match ? Number(match[1]) : null;
}

/** Whether `slide` has the element (a selection may outlive it, e.g. after undo). */
export function hasElement(slide: Slide, element: ElementId): boolean {
  const answer = answerIndex(element);
  if (answer !== null) return slide.type === "question" && answer < slide.answers.length;
  switch (element) {
    case "slide":
      return true;
    case "question":
      return slide.type === "question";
    case "title":
      return hasTitle(slide);
    case "text":
      return slide.type === "billboard" || slide.type === "explanation";
    case "picture":
      return slide.picture !== null;
    case "video":
      return slide.video !== null;
    case "sound":
      return slide.sound !== null;
    default:
      return false;
  }
}

/** The selectable element under a click on a slide; the slide itself when none. */
export function elementAt(target: EventTarget | null): ElementId {
  const marked = target instanceof Element ? target.closest<HTMLElement>("[data-element]") : null;
  const element = marked?.dataset.element;
  return element && isElementId(element) ? element : "slide";
}

function isElementId(value: string): value is ElementId {
  return ["slide", "question", "title", "text", "picture", "video", "sound"].includes(value) || /^answer-\d+$/.test(value);
}
