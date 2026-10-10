// What is behind a screen (spec BG-6): the one place that decides it, so every
// stage (projector, editor canvas, thumbnails) draws the same thing.

import type { Background, Gradient, ScreenKey, Slide, Theme, ThemeFile } from "../types";

/** One level that can give a screen its background. */
export type BackgroundLayer =
  /** A picture or video from the quiz file, with its video options (BG-4). */
  | { level: "slide"; background: Background }
  /** The slide style's gradient, which never fails to draw. */
  | { level: "fill"; gradient: Gradient }
  /** The theme's picture or video for the screen (TH-2, TH-5), or the built-in default theme's. */
  | { level: "theme"; file: ThemeFile };

/**
 * The backgrounds that can be behind `screen`, the most specific first (BG-6:
 * slide > round > quiz > theme > built-in): the slide's own, its style's
 * gradient (over the theme's, which shows through where it is transparent), the theme's,
 * which its parents fill in, and under another theme the built-in default
 * theme's `builtIn` (none until the themes are loaded). The first that loads
 * is drawn; one that fails gives way to the next (BG-9), so a theme whose
 * file is broken still has the app's own behind it. Round and quiz levels
 * join here once the quiz format has them.
 */
export function resolveBackground(slide: Slide | null, theme: Theme | null, screen: ScreenKey, builtIn: Theme | null = null): BackgroundLayer[] {
  const layers: BackgroundLayer[] = [];
  if (slide?.background) layers.push({ level: "slide", background: slide.background });
  if (slide?.style?.fill) layers.push({ level: "fill", gradient: slide.style.fill });
  if (theme) layers.push({ level: "theme", file: theme.backgrounds[screen] });
  if (builtIn && builtIn.id !== theme?.id) layers.push({ level: "theme", file: builtIn.backgrounds[screen] });
  return layers;
}
