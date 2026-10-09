import { describe, expect, it } from "vitest";
import { newSlide } from "../editor/slides";
import { previewView } from "../host/preview";
import type { Phase } from "../types";
import {
  BUILTIN_THEMES,
  contrast,
  DEFAULT_THEME_ID,
  readableText,
  resolveTheme,
  SCREENS,
  screenOf,
  themeStyle,
  withAlpha,
  type ThemeDefinition,
} from "./themes";

const COLOR = /^#[0-9a-f]{6}([0-9a-f]{2})?$/;

/** A built-in theme and test themes beside it, as installed themes will be. */
function withThemes(...themes: ThemeDefinition[]): ThemeDefinition[] {
  return [...BUILTIN_THEMES, ...themes];
}

describe("themes", () => {
  it("fills every value of every built-in theme, with colours as TH-4 writes them", () => {
    expect(new Set(BUILTIN_THEMES.map((t) => t.id)).size).toBe(BUILTIN_THEMES.length);
    for (const { id } of BUILTIN_THEMES) {
      const theme = resolveTheme(id);
      expect(theme.id).toBe(id);
      for (const color of Object.values(theme.colors)) expect(color).toMatch(COLOR);
      expect(theme.fonts.heading).toBeTruthy();
      expect(theme.fonts.body).toBeTruthy();
      for (const screen of SCREENS) expect(theme.backgrounds[screen]).toBeTruthy();
    }
  });

  it("keeps text on every coloured area readable, at 3:1 or more", () => {
    for (const { id } of BUILTIN_THEMES) {
      const { colors } = resolveTheme(id);
      const filled = [colors.bar, colors.tile, colors.correct, colors.wrong, colors.answerA, colors.answerB, colors.answerC, colors.answerD];
      for (const color of filled) expect(contrast(readableText(color), color), `${id} ${color}`).toBeGreaterThanOrEqual(3);
    }
  });

  it("takes what a theme leaves out from its parents", () => {
    const night = resolveTheme("builtin.night");
    const neon = resolveTheme("builtin.neon");
    expect(neon.colors.panel).toBe(night.colors.panel);
    expect(neon.colors.accent).not.toBe(night.colors.accent);
    expect(neon.colors.wrong).toBe(resolveTheme(DEFAULT_THEME_ID).colors.wrong);
  });

  it("gives the default theme for a theme that is not installed, and survives a broken parent chain", () => {
    expect(resolveTheme(null).id).toBe(DEFAULT_THEME_ID);
    expect(resolveTheme("cz.example.gone").id).toBe(DEFAULT_THEME_ID);
    const loop = withThemes(
      { id: "test.a", name: { cs: "A", en: "A" }, parent: "test.b", colors: { accent: "#111111" } },
      { id: "test.b", name: { cs: "B", en: "B" }, parent: "test.a", colors: { panel: "#222222" } },
      { id: "test.orphan", name: { cs: "C", en: "C" }, parent: "test.missing", colors: { text: "#333333" } },
    );
    const a = resolveTheme("test.a", loop);
    expect(a.colors).toMatchObject({ accent: "#111111", panel: "#222222" });
    expect(a.colors.bar).toBe(resolveTheme(DEFAULT_THEME_ID).colors.bar);
    expect(resolveTheme("test.orphan", loop).colors.text).toBe("#333333");
  });

  it("does not mix a theme's backgrounds with its parent's", () => {
    const themes = withThemes(
      { id: "test.own", name: { cs: "V", en: "O" }, parent: "builtin.showtime", backgrounds: { default: "own.png" } },
      { id: "test.final", name: { cs: "F", en: "F" }, parent: "builtin.showtime", backgrounds: { final: "final.png", answer: "answer.png" } },
    );
    const showtime = resolveTheme("builtin.showtime");
    // Its own default wins over the parent's final screen.
    expect(resolveTheme("test.own", themes).backgrounds.final).toBe("own.png");
    const partial = resolveTheme("test.final", themes).backgrounds;
    expect(partial).toMatchObject({ final: "final.png", answer: "answer.png" });
    // Screens it does not cover come from the parent; the question screen does not take the answer screen's.
    expect(partial.question).toBe(showtime.backgrounds.question);
    expect(partial.roundEnd).toBe(showtime.backgrounds.default);
  });

  it("picks the background for the screen the game is on", () => {
    const view = (phase: Phase) => ({ ...previewView(newSlide("question", "q"), 0, 1, ""), phase });
    expect(screenOf(view({ phase: "lobby" }))).toBe("welcome");
    expect(screenOf(view({ phase: "question", step: "closed" }))).toBe("question");
    expect(screenOf(view({ phase: "question", step: "revealed", winner: null }))).toBe("answer");
    expect(screenOf(view({ phase: "explanation" }))).toBe("answer");
    expect(screenOf(view({ phase: "billboard" }))).toBe("default");
    expect(screenOf(view({ phase: "endOfRound" }))).toBe("roundEnd");
    expect(screenOf(view({ phase: "final", step: 0 }))).toBe("final");
  });

  it("passes the theme to the stage as CSS variables", () => {
    const style = themeStyle(resolveTheme(DEFAULT_THEME_ID)) as Record<string, string>;
    expect(style["--theme-answer-3"]).toBe(resolveTheme(DEFAULT_THEME_ID).colors.answerD);
    // Yellow takes dark text, the dark bar white text.
    expect(style["--theme-answer-3-text"]).toBe("#1a1a1a");
    expect(style["--theme-bar-text"]).toBe("#ffffff");
    expect(withAlpha("#ffd21f", 0x73)).toBe("#ffd21f73");
    expect(withAlpha("#ffd21f80", 0)).toBe("#ffd21f00");
  });

  it("measures contrast as WCAG 2.2 does", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
    expect(contrast("#777777", "#777777")).toBeCloseTo(1);
  });
});
