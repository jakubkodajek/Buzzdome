// A slide's own look over its theme (quiz_core::style) as CSS: the one place
// that turns a gradient or a text style into what the stage shows, so the
// projector, the editor and the thumbnails draw the same. It sets only
// variables (`--style-*`, `--text-scale`, `--answer-fill`); slide.css owns
// every theme value they fall back to.

import type { CSSProperties } from "react";
import type { Gradient, GradientKind, SlideStyle, TextStyle } from "../types";
import { cssColor, opacity } from "./color";
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

/**
 * Whether the theme draws each text bold and with a shadow, as slide.css
 * does (a test reads it there), for the editor to show its look as set.
 */
export const THEME_TEXT: Record<StyledText, { bold: boolean; shadow: boolean }> = {
  title: { bold: true, shadow: true },
  question: { bold: true, shadow: true },
  text: { bold: false, shadow: false },
  answer: { bold: true, shadow: false },
};

/**
 * The variables that give `text` its style; empty for none. Only what the
 * style changes is set, and slide.css reads each over the text's theme
 * look ("Styled texts"), so the theme (and the screen's own rules, such as
 * a revealed answer) keeps the rest. An answer's fill goes to
 * `--answer-fill`, under the theme's sheen; another text's fill becomes a
 * box around it. Text on a fill of its own and in no colour of its own is
 * made readable on the fill (the theme's colour suits the theme's fill),
 * unless the fill mostly shows through: on average over its ends, its
 * colours' own opacity and the transparency leave less than half of it.
 */
export function textStyleCss(style: TextStyle | undefined, text: StyledText): CSSProperties {
  if (!style) return {};
  const css: Record<string, string | number> = {};
  if (style.color) css["--style-color"] = cssColor(style.color);
  if (style.font) css["--style-font"] = `"${style.font}"`;
  if (style.size !== undefined) css["--text-scale"] = style.size / 100;
  if (style.bold !== undefined) css["--style-weight"] = style.bold ? "var(--text-bold)" : "normal";
  if (style.italic !== undefined) css["--style-italic"] = style.italic ? "italic" : "normal";
  if (style.underline !== undefined) css["--style-underline"] = style.underline ? "underline" : "none";
  if (style.align) {
    css["--style-align"] = style.align;
    // A text as wide as the screen, so its alignment shows; an answer's text sits in its panel.
    if (text !== "answer") css["--style-stretch"] = "stretch";
  }
  if (style.shadow !== undefined) css["--style-shadow"] = style.shadow ? "var(--text-shadow)" : "none";
  if (style.shadowColor) css["--style-shadow-color"] = cssColor(style.shadowColor);
  const transparency = style.transparency ?? 0;
  const covers = style.fill && (opacity(style.fill.from, transparency) + opacity(style.fill.to, transparency)) / 2 > 0.5;
  if (style.fill && !style.color && covers) css["--style-color"] = readableText(style.fill.from, style.fill.to);
  if (text === "answer") {
    if (style.fill) css["--answer-fill"] = gradientCss(style.fill, transparency);
    else if (transparency > 0) css["--answer-fill"] = `color-mix(in srgb, var(--answer) ${100 - transparency}%, transparent)`;
  } else if (style.fill) {
    css["--style-fill"] = gradientCss(style.fill, transparency);
    css["--style-boxed"] = 1;
  }
  return css as CSSProperties;
}
