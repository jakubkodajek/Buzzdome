import { describe, expect, it } from "vitest";
import BUILTIN_THEMES from "../bindings/BuiltinThemes.json";
import { newSlide, setBackground, setSlideFill } from "../editor/slides";
import type { Theme } from "../types";
import { resolveBackground } from "./background";

const theme: Theme = BUILTIN_THEMES[0];

describe("resolveBackground", () => {
  it("puts the slide's own background before the theme's", () => {
    const slide = setBackground(newSlide("billboard", "a"), "media/stage.mp4");
    const layers = resolveBackground(slide, theme, "default");
    expect(layers.map((layer) => layer.level)).toEqual(["slide", "theme"]);
    expect(layers[0]).toMatchObject({ background: { file: "media/stage.mp4", repeat: true, muted: true, sync: false } });
  });

  it("puts the style's gradient between the slide's own background and the theme's", () => {
    const gradient = { from: "#000000", to: "#336699", kind: "radial" } as const;
    const slide = setSlideFill(setBackground(newSlide("question", "q"), "media/stage.png"), gradient);
    expect(resolveBackground(slide, theme, "question").map((layer) => layer.level)).toEqual(["slide", "fill", "theme"]);
    expect(resolveBackground(setBackground(slide, null), theme, "question")[0]).toEqual({ level: "fill", gradient });
  });

  it("ends with the theme's background for the screen", () => {
    expect(resolveBackground(newSlide("question", "q"), theme, "question")).toEqual([{ level: "theme", file: theme.backgrounds.question }]);
    expect(resolveBackground(null, theme, "leaderboard")).toEqual([{ level: "theme", file: theme.backgrounds.leaderboard }]);
  });

  it("puts the built-in default under another theme, once", () => {
    const other: Theme = BUILTIN_THEMES[1];
    expect(resolveBackground(null, other, "question", theme)).toEqual([
      { level: "theme", file: other.backgrounds.question },
      { level: "theme", file: theme.backgrounds.question },
    ]);
    expect(resolveBackground(null, theme, "question", theme)).toEqual([{ level: "theme", file: theme.backgrounds.question }]);
  });

  it("has only the slide's own until the themes are loaded", () => {
    const slide = setBackground(newSlide("billboard", "a"), "media/stage.png");
    expect(resolveBackground(slide, null, "default").map((layer) => layer.level)).toEqual(["slide"]);
    expect(resolveBackground(null, null, "default")).toEqual([]);
  });
});
