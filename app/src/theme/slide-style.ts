// A slide's own look over its theme (quiz_core::style) as CSS: the one place
// that turns a gradient or a text style into what the stage shows, so the
// projector, the editor and the thumbnails draw the same. Sizes scale the
// theme's through `--text-scale` (slide.css).

import type { CSSProperties } from "react";
import type { Gradient, GradientKind, SlideStyle, TextAlign, TextStyle } from "../types";
import { cssColor } from "./color";
import { readableText } from "./themes";

/** The texts a style applies to, as slide.css draws them. */
export type StyledText = "title" | "question" | "text" | "answer";

/** The part of a slide's style for `text`: a title and a question share the headline's. */
export function styleOf(style: SlideStyle | undefined, text: StyledText): TextStyle | undefined {
  switch (text) {
    case "title":
    case "question":
      return style?.headline;
    case "text":
      return style?.text;
    case "answer":
      return style?.answers;
  }
}

const GRADIENT_DIRECTIONS: Record<Exclude<GradientKind, "radial">, string> = {
  vertical: "to bottom",
  horizontal: "to right",
  diagonal: "to bottom right",
  diagonalUp: "to top right",
};

/** The gradient for a CSS `background`, `transparency` percent of it showing through. */
export function gradientCss({ from, to, kind }: Gradient, transparency = 0): string {
  const stops = `${cssColor(from, transparency)}, ${cssColor(to, transparency)}`;
  return kind === "radial" ? `radial-gradient(circle, ${stops})` : `linear-gradient(${GRADIENT_DIRECTIONS[kind]}, ${stops})`;
}

/** How the theme draws each text: its font, and whether it is bold and has a shadow. */
export const THEME_TEXT: Record<StyledText, { font: "heading" | "body"; bold: boolean; shadow: boolean }> = {
  title: { font: "heading", bold: true, shadow: true },
  question: { font: "body", bold: true, shadow: true },
  text: { font: "body", bold: false, shadow: false },
  answer: { font: "body", bold: true, shadow: false },
};

const JUSTIFY: Record<TextAlign, string> = { left: "flex-start", center: "center", right: "flex-end" };

/** A drop shadow as the theme's question has one, in another colour. */
const SHADOW = "0 4px 18px";

/**
 * The inline style that gives `text` its style; empty for none. Only what
 * the style changes is set, so the theme (and the screen's own rules, such
 * as a revealed answer) keeps the rest. An answer's fill goes to
 * `--answer-fill`, under the theme's sheen; another text's fill becomes a
 * box around it. Text on a fill of its own and in no colour of its own is
 * made readable on the fill (the theme's colour suits the theme's fill),
 * unless the fill mostly shows through.
 */
export function textStyleCss(style: TextStyle | undefined, text: StyledText): CSSProperties {
  if (!style) return {};
  const theme = THEME_TEXT[text];
  const css: Record<string, string | number> = {};
  if (style.color) css.color = cssColor(style.color);
  if (style.font) css.fontFamily = `"${style.font}", var(--theme-font-${theme.font})`;
  if (style.size !== undefined) css["--text-scale"] = style.size / 100;
  if (style.bold !== undefined && style.bold !== theme.bold) css.fontWeight = style.bold ? "bold" : "normal";
  if (style.italic !== undefined) css.fontStyle = style.italic ? "italic" : "normal";
  if (style.underline !== undefined) css.textDecoration = style.underline ? "underline" : "none";
  if (style.align) {
    css.textAlign = style.align;
    // A text as wide as the screen, so its alignment shows; an answer's text sits in its panel.
    if (text === "answer") css.justifyContent = JUSTIFY[style.align];
    else css.alignSelf = "stretch";
  }
  const shadow = style.shadow ?? theme.shadow;
  if (!shadow && theme.shadow) css.textShadow = "none";
  else if (shadow && (style.shadowColor || !theme.shadow)) css.textShadow = `${SHADOW} ${style.shadowColor ? cssColor(style.shadowColor) : "var(--theme-shadow)"}`;
  const transparency = style.transparency ?? 0;
  if (style.fill && !style.color && transparency < 50) css.color = readableText(style.fill.from, style.fill.to);
  if (text === "answer") {
    if (style.fill) css["--answer-fill"] = gradientCss(style.fill, transparency);
    else if (transparency > 0) css["--answer-fill"] = `color-mix(in srgb, var(--answer) ${100 - transparency}%, transparent)`;
  } else if (style.fill) {
    css.background = gradientCss(style.fill, transparency);
    css.padding = "0.15em 0.5em";
    css.borderRadius = 22;
  }
  return css as CSSProperties;
}
