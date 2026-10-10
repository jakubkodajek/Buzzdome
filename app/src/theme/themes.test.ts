import { describe, expect, it } from "vitest";
import { BUILTIN_THEMES } from "../bindings/BuiltinThemes";
import { DEFAULT_THEME_ID } from "../bindings/Constants";
import { newSlide } from "../editor/slides";
import { previewView } from "../host/preview";
import type { Phase } from "../types";
import { fontFamily } from "./fonts";
import { contrast, isThemeMissing, pickTheme, readableText, screenOf, themeStyle, withAlpha } from "./themes";

const THEMES = BUILTIN_THEMES;
const classic = THEMES.find((theme) => theme.id === DEFAULT_THEME_ID)!;

describe("themes", () => {
  it("keeps text on every coloured area of a built-in theme readable, at 3:1 or more", () => {
    for (const { id, colors } of THEMES) {
      const filled = [colors.bar, colors.tile, colors.correct, colors.wrong, colors.answerA, colors.answerB, colors.answerC, colors.answerD, colors.answerE, colors.answerF];
      for (const color of filled) expect(contrast(readableText(color), color), `${id} ${color}`).toBeGreaterThanOrEqual(3);
    }
  });

  it("picks text readable on both ends of a gradient where one text is", () => {
    expect(readableText("#0B1F4A", "#7A1E1E")).toBe("#ffffff");
    expect(readableText("#FFFFFF", "#FFE066")).toBe("#1a1a1a");
    expect(readableText("#1E3C72", "#FFFFFF")).toBe("#1a1a1a");
  });

  it("picks the quiz's theme, and the default one for a theme that is not installed", () => {
    expect(pickTheme(THEMES, "builtin.neon")?.id).toBe("builtin.neon");
    expect(pickTheme(THEMES, null)?.id).toBe(DEFAULT_THEME_ID);
    expect(pickTheme(THEMES, "cz.example.gone")?.id).toBe(DEFAULT_THEME_ID);
    expect([isThemeMissing(THEMES, "cz.example.gone"), isThemeMissing(THEMES, "builtin.neon"), isThemeMissing(THEMES, null)]).toEqual([true, false, false]);
    expect(isThemeMissing([], "cz.example.gone"), "not before the themes are loaded").toBe(false);
    // Before the themes are loaded there is none.
    expect(pickTheme([], "builtin.neon")).toBeNull();
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
    const style = themeStyle(classic) as Record<string, string>;
    expect(style["--theme-answer-3"]).toBe(classic.colors.answerD);
    expect(style["--theme-answer-5"]).toBe(classic.colors.answerF);
    // Yellow takes dark text, the dark bar white text.
    expect(style["--theme-answer-3-text"]).toBe("#1a1a1a");
    expect(style["--theme-bar-text"]).toBe("#ffffff");
    // The theme's font file first, then fonts for letters it does not have.
    expect(style["--theme-font-heading"]).toMatch(new RegExp(`^${JSON.stringify(fontFamily(classic.fonts.heading))}, .*sans-serif$`));
    // Rounded corners and titles as written, unless a theme says otherwise.
    expect([style["--theme-roundness"], style["--theme-heading-case"]]).toEqual(["1", "none"]);
    const bakchus = themeStyle(THEMES.find((theme) => theme.id === "builtin.bakchus")!) as Record<string, string>;
    expect([bakchus["--theme-roundness"], bakchus["--theme-heading-case"]]).toEqual(["0", "uppercase"]);
    expect(withAlpha("#ffd21f", 0x73)).toBe("#ffd21f73");
    expect(withAlpha("#ffd21f80", 0)).toBe("#ffd21f00");
  });

  it("names a font by its file, so themes that share one share the family", () => {
    const night = THEMES.find((theme) => theme.id === "builtin.night")!;
    expect(fontFamily(night.fonts.body)).toBe(fontFamily(classic.fonts.body));
    expect(fontFamily(classic.fonts.heading)).toBe(`theme:${classic.fonts.heading.root}${classic.fonts.heading.path}`);
  });

  it("measures contrast as WCAG 2.2 does", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
    expect(contrast("#777777", "#777777")).toBeCloseTo(1);
  });
});
