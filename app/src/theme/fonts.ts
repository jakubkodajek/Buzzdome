// Theme fonts are files of the theme packages (spec 5.2). Each file is
// registered with the document once, under a family name of its own.

import { themeFileUrl } from "../media";
import type { Theme, ThemeFile } from "../types";

/** The CSS family name a theme's font file is registered under. */
export function fontFamily(file: ThemeFile): string {
  return `theme:${file.root}${file.path}`;
}

const registered = new Set<string>();

/**
 * Registers the theme's font files with the document, each once; the
 * browser loads one when text first uses it. A theme font is meant to be
 * variable, so it is declared for every weight (CSS Fonts 4, `font-weight`
 * ranges): a static font shows its one weight everywhere rather than a
 * faked bold.
 */
export function registerFonts(theme: Theme): void {
  for (const file of [theme.fonts.heading, theme.fonts.body]) {
    const family = fontFamily(file);
    if (registered.has(family)) continue;
    registered.add(family);
    const face = new FontFace(family, `url(${JSON.stringify(themeFileUrl(file))})`, { weight: "1 1000" });
    document.fonts.add(face);
    face.loaded.catch((e: unknown) => console.error("theme font:", family, e));
  }
}
