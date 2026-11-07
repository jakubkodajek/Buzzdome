// Text that would not fit its box on the stage is drawn smaller, down to a
// floor: a long question or answer is never cut off by its tile or
// pushed off the stage. The projector, the editor's canvas and thumbnails
// draw slides with the same code, so the author sees what the room will.

import { useLayoutEffect, type RefObject } from "react";

/** The smallest share of its size a text is drawn at to fit. */
export const FIT_FLOOR = 0.6;

/** How much smaller each try draws the text. */
const FIT_STEP = 0.05;

/**
 * The largest scale from 1 down in steps of `FIT_STEP` at which nothing
 * `overflows`; `FIT_FLOOR` when even that does not fit.
 */
export function fitScale(overflows: (scale: number) => boolean): number {
  for (let step = 0; ; step += 1) {
    const scale = Math.round((1 - step * FIT_STEP) * 100) / 100;
    if (scale <= FIT_FLOOR || !overflows(scale)) return Math.max(scale, FIT_FLOOR);
  }
}

/** Whether `element`'s content is taller than its box (a pixel of rounding allowed). */
function tallerThanBox(element: Element): boolean {
  return element.scrollHeight > element.clientHeight + 1;
}

/**
 * Whether a part of `screen` laid out in its flow ends below its padding.
 * Parts placed over it (the buzz bar, which slides in from below) and
 * transforms (a right answer drawn larger) do not count.
 */
function pastScreen(screen: HTMLElement): boolean {
  const bottom = screen.clientHeight - parseFloat(getComputedStyle(screen).paddingBottom);
  return [...screen.children].some(
    (part) => part instanceof HTMLElement && getComputedStyle(part).position !== "absolute" && part.offsetTop + part.offsetHeight > bottom + 1,
  );
}

/**
 * Sets `--fit` on the screen `screen` refers to: the scale its texts are
 * drawn at (slide.css multiplies their sizes by it) so that the screen and
 * every one of its `boxes` (a selector) hold their content. Measured again
 * when `content` changes and once a font has loaded.
 */
export function useFitText(screen: RefObject<HTMLElement | null>, boxes: string, content: string) {
  useLayoutEffect(() => {
    const element = screen.current;
    if (!element) return;
    const fit = () => {
      const scale = fitScale((tried) => {
        element.style.setProperty("--fit", String(tried));
        return pastScreen(element) || [...element.querySelectorAll(boxes)].some(tallerThanBox);
      });
      element.style.setProperty("--fit", String(scale));
    };
    fit();
    document.fonts?.addEventListener("loadingdone", fit);
    return () => document.fonts?.removeEventListener("loadingdone", fit);
  }, [screen, boxes, content]);
}
