import { describe, expect, it } from "vitest";
import { newSlide, setBackground } from "../editor/slides";
import { resolveBackground } from "./background";
import { DEFAULT_THEME_ID, resolveTheme } from "./themes";

const theme = resolveTheme(DEFAULT_THEME_ID);

describe("resolveBackground", () => {
  it("puts the slide's own background before the theme's", () => {
    const slide = setBackground(newSlide("billboard", "a"), "media/stage.mp4");
    const layers = resolveBackground(slide, theme, "default");
    expect(layers.map((layer) => layer.level)).toEqual(["slide", "theme"]);
    expect(layers[0]).toMatchObject({ background: { file: "media/stage.mp4", repeat: true, muted: true, sync: false } });
  });

  it("always ends with the theme's background for the screen", () => {
    expect(resolveBackground(newSlide("question", "q"), theme, "question")).toEqual([{ level: "theme", url: theme.backgrounds.question }]);
    expect(resolveBackground(null, theme, "leaderboard")).toEqual([{ level: "theme", url: theme.backgrounds.leaderboard }]);
  });
});
