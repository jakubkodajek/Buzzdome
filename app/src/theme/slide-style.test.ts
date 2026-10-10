import { describe, expect, it } from "vitest";
import { gradientCss, styleOf, textStyleCss } from "./slide-style";

describe("slide styles as CSS", () => {
  it("draws each kind of gradient, with its transparency", () => {
    const gradient = { from: "#000000", to: "#FFFFFF" };
    expect(gradientCss({ ...gradient, kind: "vertical" })).toBe("linear-gradient(to bottom, rgba(0, 0, 0, 1), rgba(255, 255, 255, 1))");
    expect(gradientCss({ ...gradient, kind: "diagonalUp" }, 40)).toBe("linear-gradient(to top right, rgba(0, 0, 0, 0.6), rgba(255, 255, 255, 0.6))");
    expect(gradientCss({ ...gradient, kind: "radial" })).toBe("radial-gradient(circle, rgba(0, 0, 0, 1), rgba(255, 255, 255, 1))");
  });

  it("gives a title and a question the headline's style", () => {
    const style = { headline: { color: "#FF0000" }, text: { italic: true }, answers: { bold: false } };
    expect(styleOf(style, "title")).toBe(style.headline);
    expect(styleOf(style, "question")).toBe(style.headline);
    expect(styleOf(style, "text")).toBe(style.text);
    expect(styleOf(style, "answer")).toBe(style.answers);
    expect(styleOf(undefined, "answer")).toBeUndefined();
  });

  it("sets only what the style changes", () => {
    expect(textStyleCss(undefined, "title")).toEqual({});
    expect(textStyleCss({}, "answer")).toEqual({});
    expect(textStyleCss({ color: "#102030", font: "Georgia", size: 150, italic: true, underline: false }, "title")).toEqual({
      color: "rgba(16, 32, 48, 1)",
      fontFamily: '"Georgia", var(--theme-font-heading)',
      "--text-scale": 1.5,
      fontStyle: "italic",
      textDecoration: "none",
    });
    expect(textStyleCss({ font: "Georgia" }, "question").fontFamily).toBe('"Georgia", var(--theme-font-body)');
  });

  it("changes bold and the shadow only where the theme has them otherwise", () => {
    // Titles are bold and have a shadow in the theme; texts have neither.
    expect(textStyleCss({ bold: true, shadow: true }, "title")).toEqual({});
    expect(textStyleCss({ bold: false, shadow: false }, "title")).toEqual({ fontWeight: "normal", textShadow: "none" });
    expect(textStyleCss({ bold: true, shadow: true }, "text")).toEqual({ fontWeight: "bold", textShadow: "0 4px 18px var(--theme-shadow)" });
    expect(textStyleCss({ shadow: false }, "text")).toEqual({});
    expect(textStyleCss({ shadowColor: "#00FF00" }, "question")).toEqual({ textShadow: "0 4px 18px rgba(0, 255, 0, 1)" });
    // A colour for a shadow that is off draws none.
    expect(textStyleCss({ shadowColor: "#00FF00" }, "answer")).toEqual({});
  });

  it("aligns a text across the screen and an answer in its panel", () => {
    expect(textStyleCss({ align: "left" }, "text")).toEqual({ textAlign: "left", alignSelf: "stretch" });
    expect(textStyleCss({ align: "right" }, "answer")).toEqual({ textAlign: "right", justifyContent: "flex-end" });
  });

  it("puts an answer's fill under the theme's sheen and boxes another text's", () => {
    const fill = { from: "#000000", to: "#000000", kind: "horizontal" } as const;
    expect(textStyleCss({ fill, transparency: 50 }, "answer")).toEqual({
      "--answer-fill": "linear-gradient(to right, rgba(0, 0, 0, 0.5), rgba(0, 0, 0, 0.5))",
    });
    // Without a fill of its own, an answer's theme colour shows through.
    expect(textStyleCss({ transparency: 30 }, "answer")).toEqual({ "--answer-fill": "color-mix(in srgb, var(--answer) 70%, transparent)" });
    expect(textStyleCss({ fill }, "title")).toMatchObject({ background: "linear-gradient(to right, rgba(0, 0, 0, 1), rgba(0, 0, 0, 1))", borderRadius: 22 });
    // A title has no fill to make transparent.
    expect(textStyleCss({ transparency: 30 }, "title")).toEqual({});
  });
});
