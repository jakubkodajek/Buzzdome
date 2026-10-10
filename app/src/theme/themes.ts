// Drawing with a theme (spec section 5.2). Themes are data: packages that
// Rust reads, fills in from their parents and hands over as `Theme`s
// (`list_themes`). This turns one into what a stage needs.

import type { CSSProperties } from "react";
import { DEFAULT_THEME_ID } from "../bindings/Constants";
import type { ColorKey, GameView, ScreenKey, Theme } from "../types";
import { parseColor } from "./color";
import { fontFamily } from "./fonts";

/**
 * The theme `id` among the installed `themes`; the default theme for an id
 * that is not installed, so a missing look never stops a show. Null only
 * before the themes are loaded.
 */
export function pickTheme(themes: readonly Theme[], id: string | null): Theme | null {
  return themes.find((theme) => theme.id === id) ?? themes.find((theme) => theme.id === DEFAULT_THEME_ID) ?? null;
}

/** The screen a game state shows, for its background. */
export function screenOf(view: GameView): ScreenKey {
  switch (view.phase.phase) {
    case "lobby":
      return "welcome";
    case "question":
      return view.phase.step === "revealed" ? "answer" : "question";
    case "explanation":
      return "answer";
    case "endOfRound":
      return "roundEnd";
    case "final":
      return "final";
    case "billboard":
      return "default";
  }
}

const ANSWER_KEYS: readonly ColorKey[] = ["answerA", "answerB", "answerC", "answerD", "answerE", "answerF"];

/** Fonts for text a theme font has no letters for. */
const FALLBACK_FONTS = '"Segoe UI", "Helvetica Neue", Arial, sans-serif';

/** A theme font for `font-family`, followed by the fallback fonts. */
function fontStack(family: string): string {
  return `${JSON.stringify(family)}, ${FALLBACK_FONTS}`;
}

/**
 * The theme as CSS custom properties for a stage (slide.css). Text colours on
 * coloured areas are chosen here, so every theme stays readable.
 */
export function themeStyle(theme: Theme): CSSProperties {
  const { colors, fonts } = theme;
  const vars: Record<string, string> = {
    "--theme-text": colors.text,
    "--theme-shadow": colors.shadow,
    "--theme-glow": colors.glow,
    "--theme-panel": colors.panel,
    "--theme-bar": colors.bar,
    "--theme-bar-edge": colors.barEdge,
    "--theme-bar-text": readableText(colors.bar),
    "--theme-tile": colors.tile,
    "--theme-tile-text": readableText(colors.tile),
    "--theme-accent": colors.accent,
    "--theme-accent-glow": withAlpha(colors.accent, 0x73),
    "--theme-correct": colors.correct,
    "--theme-correct-text": readableText(colors.correct),
    "--theme-wrong": colors.wrong,
    "--theme-wrong-text": readableText(colors.wrong),
    "--theme-font-heading": fontStack(fontFamily(fonts.heading)),
    "--theme-font-body": fontStack(fontFamily(fonts.body)),
  };
  ANSWER_KEYS.forEach((key, i) => {
    vars[`--theme-answer-${i}`] = colors[key];
    vars[`--theme-answer-${i}-text`] = readableText(colors[key]);
  });
  return vars as CSSProperties;
}

/** `color` (`#rrggbb` or `#rrggbbaa`) with its alpha set to `alpha` (0-255). */
export function withAlpha(color: string, alpha: number): string {
  return color.slice(0, 7) + alpha.toString(16).padStart(2, "0");
}

const LIGHT_TEXT = "#ffffff";
const DARK_TEXT = "#1a1a1a";

/**
 * White text when it is readable on `background`, dark text otherwise.
 * Readable means a contrast of at least 3:1, the WCAG minimum for large
 * text, which all text on a stage is (WCAG 2.2, 1.4.3).
 */
export function readableText(background: string): string {
  return contrast(LIGHT_TEXT, background) >= 3 ? LIGHT_TEXT : DARK_TEXT;
}

/** WCAG 2.2 contrast ratio of two colours, 1 to 21; alpha is ignored. */
export function contrast(a: string, b: string): number {
  const [lighter, darker] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (lighter + 0.05) / (darker + 0.05);
}

/** WCAG 2.2 relative luminance of `#rrggbb` or `#rrggbbaa`; black for anything else. */
function luminance(hex: string): number {
  const { r, g, b } = parseColor(hex) ?? { r: 0, g: 0, b: 0 };
  const [red, green, blue] = [r, g, b].map((value) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}
