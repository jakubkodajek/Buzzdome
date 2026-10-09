// The looks a quiz can have on screen (spec section 5.2). A theme is data:
// colours, fonts and a background per screen. It lists only what differs
// from its parent; `resolveTheme` fills in the rest. The built-in themes are
// written here; installed theme packages will be converted to the same form.

import "@fontsource-variable/exo-2";
import "@fontsource-variable/inter";
import "@fontsource-variable/nunito";
import "@fontsource-variable/oswald";
import type { CSSProperties } from "react";
import type { Lang } from "../i18n";
import type { GameView } from "../types";
import classicBackground from "./backgrounds/classic.svg";
import neonBackground from "./backgrounds/neon.svg";
import nightBackground from "./backgrounds/night.svg";
import showtimeFinal from "./backgrounds/showtime-final.svg";
import showtimeBackground from "./backgrounds/showtime.svg";
import studioBackground from "./backgrounds/studio.svg";

/** Colours as `#rrggbb` or `#rrggbbaa` (TH-4). */
export interface ThemeColors {
  /** Text on the background. */
  text: string;
  /** Shadow under text on the background, to keep it readable. */
  shadow: string;
  /** Glow around slide titles. */
  glow: string;
  /** Boxes placed on the background. */
  panel: string;
  /** The bars across the top and bottom of a screen. */
  bar: string;
  /** The bright line along a bar's edge. */
  barEdge: string;
  /** Leaderboard rows. */
  tile: string;
  /** Scores and counters. */
  accent: string;
  answerA: string;
  answerB: string;
  answerC: string;
  answerD: string;
  correct: string;
  wrong: string;
}

/** CSS font families, most wanted first. */
export interface ThemeFonts {
  /** Titles, names and big numbers. */
  heading: string;
  /** Questions, answers and other text. */
  body: string;
}

/** The screens a theme can give its own background (TH-5). */
export const SCREENS = ["default", "welcome", "question", "answer", "leaderboard", "roundEnd", "final"] as const;
export type ScreenKey = (typeof SCREENS)[number];

/** Where a screen without its own background takes one from (TH-5). */
const SCREEN_FALLBACK: Record<Exclude<ScreenKey, "default">, ScreenKey> = {
  welcome: "default",
  question: "default",
  answer: "question",
  leaderboard: "default",
  roundEnd: "leaderboard",
  final: "default",
};

/** `screen` and the screens it falls back to, ending at "default". */
function fallbackPath(screen: ScreenKey): ScreenKey[] {
  const path: ScreenKey[] = [screen];
  for (let key = screen; key !== "default"; ) {
    key = SCREEN_FALLBACK[key];
    path.push(key);
  }
  return path;
}

/** A theme as written: everything but `id` and `name` may come from `parent`. */
export interface ThemeDefinition {
  id: string;
  name: Record<Lang, string>;
  /** Missing only on the root theme. */
  parent?: string;
  colors?: Partial<ThemeColors>;
  fonts?: Partial<ThemeFonts>;
  /** Image URLs. A theme that sets any background replaces its parent's set. */
  backgrounds?: Partial<Record<ScreenKey, string>>;
}

/** A theme with every value filled in, ready to draw with. */
export interface Theme {
  id: string;
  name: Record<Lang, string>;
  colors: ThemeColors;
  fonts: ThemeFonts;
  backgrounds: Record<ScreenKey, string>;
}

const SANS = '"Segoe UI", "Helvetica Neue", Arial, sans-serif';

/** The root of every theme: it sets every value, so nothing is ever missing. */
const CLASSIC = {
  id: "builtin.classic",
  name: { cs: "Klasický", en: "Classic" },
  colors: {
    text: "#ffffff",
    shadow: "#000000b3",
    glow: "#286eff8c",
    panel: "#ffffff24",
    bar: "#1b4aa6",
    barEdge: "#5d94ff",
    tile: "#1c2640",
    accent: "#ffd21f",
    answerA: "#1590e0",
    answerB: "#ec6f12",
    answerC: "#3fa31c",
    answerD: "#f2cd0c",
    correct: "#25b864",
    wrong: "#e23d2b",
  },
  fonts: { heading: `"Inter Variable", ${SANS}`, body: `"Inter Variable", ${SANS}` },
  backgrounds: { default: classicBackground },
} satisfies ThemeDefinition;

/** Built-in themes, in gallery order. The first is the default. */
export const BUILTIN_THEMES: readonly ThemeDefinition[] = [
  CLASSIC,
  {
    id: "builtin.studio",
    name: { cs: "Studio", en: "Studio" },
    parent: CLASSIC.id,
    colors: {
      glow: "#2dd4bf80",
      bar: "#0f6e68",
      barEdge: "#5eead4",
      tile: "#123f4a",
      accent: "#fde047",
      answerA: "#2f6fe4",
      answerB: "#e8620c",
      answerC: "#1d9a4b",
      answerD: "#eab308",
    },
    fonts: { heading: `"Nunito Variable", ${SANS}`, body: `"Nunito Variable", ${SANS}` },
    backgrounds: { default: studioBackground },
  },
  {
    id: "builtin.night",
    name: { cs: "Noc", en: "Night" },
    parent: CLASSIC.id,
    colors: {
      glow: "#ffffff38",
      panel: "#ffffff1a",
      bar: "#23262e",
      barEdge: "#8b93a5",
      tile: "#1b1d23",
      accent: "#f5c542",
      answerA: "#2a73c0",
      answerB: "#c9661c",
      answerC: "#2f8a49",
      answerD: "#d6b52c",
    },
    backgrounds: { default: nightBackground },
  },
  {
    id: "builtin.showtime",
    name: { cs: "Show", en: "Showtime" },
    parent: CLASSIC.id,
    colors: {
      glow: "#ff4fd880",
      panel: "#ffffff1f",
      bar: "#4a1270",
      barEdge: "#f5c542",
      tile: "#2e0b46",
      accent: "#ffd34d",
      answerA: "#2563eb",
      answerB: "#c81e6e",
      answerC: "#15803d",
      answerD: "#f59e0b",
    },
    fonts: { heading: `"Oswald Variable", "Arial Narrow", ${SANS}` },
    backgrounds: { default: showtimeBackground, final: showtimeFinal },
  },
  {
    id: "builtin.neon",
    name: { cs: "Neon", en: "Neon" },
    parent: "builtin.night",
    colors: {
      glow: "#00e5ffb3",
      bar: "#0b0716",
      barEdge: "#00e5ff",
      tile: "#120a24",
      accent: "#ff4fe1",
      answerA: "#0077cc",
      answerB: "#d94f00",
      answerC: "#00a35a",
      answerD: "#ffe600",
      correct: "#00c46a",
    },
    fonts: { heading: `"Exo 2 Variable", ${SANS}`, body: `"Exo 2 Variable", ${SANS}` },
    backgrounds: { default: neonBackground },
  },
];

export const DEFAULT_THEME_ID = CLASSIC.id;

/**
 * The theme `id` with every value filled in from its parents. An id that is
 * not installed gives the default theme: a missing look never stops a show.
 */
export function resolveTheme(id: string | null, themes: readonly ThemeDefinition[] = BUILTIN_THEMES): Theme {
  const byId = new Map(themes.map((theme) => [theme.id, theme]));
  const chain: ThemeDefinition[] = [];
  let next: ThemeDefinition = byId.get(id ?? DEFAULT_THEME_ID) ?? CLASSIC;
  // A parent that is missing or already in the chain (a loop) ends it at the root.
  while (!chain.includes(next)) {
    chain.push(next);
    next = (next.parent && byId.get(next.parent)) || CLASSIC;
  }
  const rootFirst = [...chain].reverse();
  return {
    id: chain[0].id,
    name: chain[0].name,
    colors: Object.assign({}, CLASSIC.colors, ...rootFirst.map((theme) => theme.colors)),
    fonts: Object.assign({}, CLASSIC.fonts, ...rootFirst.map((theme) => theme.fonts)),
    backgrounds: Object.fromEntries(SCREENS.map((screen) => [screen, backgroundFor(chain, screen)])) as Record<ScreenKey, string>,
  };
}

/**
 * The background of `screen`: the first theme in `chain` (the theme, then
 * its parents) that has one for the screen or a screen it falls back to.
 * So a theme that sets only "default" is not mixed with its parent's others.
 */
function backgroundFor(chain: readonly ThemeDefinition[], screen: ScreenKey): string {
  const path = fallbackPath(screen);
  for (const theme of chain) {
    const url = path.map((key) => theme.backgrounds?.[key]).find(Boolean);
    if (url) return url;
  }
  return CLASSIC.backgrounds.default;
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

const ANSWER_KEYS = ["answerA", "answerB", "answerC", "answerD"] as const;

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
    "--theme-font-heading": fonts.heading,
    "--theme-font-body": fonts.body,
  };
  ANSWER_KEYS.forEach((key, i) => {
    vars[`--theme-answer-${i}`] = colors[key];
    vars[`--theme-answer-${i}-text`] = readableText(colors[key]);
  });
  return vars as CSSProperties;
}

/** The background of `screen` as inline style. */
export function backgroundStyle(theme: Theme, screen: ScreenKey): CSSProperties {
  return { backgroundImage: `url("${theme.backgrounds[screen]}")` };
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

/** WCAG 2.2 relative luminance of `#rrggbb` or `#rrggbbaa`. */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const channel = parseInt(hex.slice(at, at + 2), 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
