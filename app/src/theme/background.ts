// What is behind a screen (spec BG-6): the one place that decides it, so every
// stage (projector, editor canvas, thumbnails) draws the same thing.

import type { Background, ScreenKey, Slide, Theme, ThemeFile } from "../types";

/** One level that can give a screen its background. */
export type BackgroundLayer =
  /** A picture or video from the quiz file, with its video options (BG-4). */
  | { level: "slide"; background: Background }
  /** The theme's picture or video for the screen (TH-2, TH-5). */
  | { level: "theme"; file: ThemeFile };

/**
 * The backgrounds that can be behind `screen`, the most specific first (BG-6:
 * slide > round > quiz > theme > built-in), ending with the theme's, which
 * its parents and the built-in default always fill in (none until the themes
 * are loaded). The first that loads is drawn; one that fails gives way to
 * the next (BG-9). Round and quiz levels join here once the quiz format has
 * them.
 */
export function resolveBackground(slide: Slide | null, theme: Theme | null, screen: ScreenKey): BackgroundLayer[] {
  const layers: BackgroundLayer[] = [];
  if (slide?.background) layers.push({ level: "slide", background: slide.background });
  if (theme) layers.push({ level: "theme", file: theme.backgrounds[screen] });
  return layers;
}
