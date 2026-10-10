import { describe, expect, it } from "vitest";
// @ts-expect-error type error without @types/node package
import { readFileSync } from "node:fs";
import { gradientCss, styleOf, textStyleCss, THEME_TEXT, type StyledText } from "./slide-style";

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

  it("sets only variables, and only for what the style changes", () => {
    expect(textStyleCss(undefined, "title")).toEqual({});
    expect(textStyleCss({}, "answer")).toEqual({});
    expect(textStyleCss({ color: "#102030", font: "Georgia", size: 150, italic: true, underline: false }, "title")).toEqual({
      "--style-color": "rgba(16, 32, 48, 1)",
      "--style-font": '"Georgia"',
      "--text-scale": 1.5,
      "--style-italic": "italic",
      "--style-underline": "none",
    });
  });

  it("turns bold and the shadow on as the text's own, and off", () => {
    expect(textStyleCss({ bold: true, shadow: true }, "title")).toEqual({ "--style-weight": "var(--text-bold)", "--style-shadow": "var(--text-shadow)" });
    expect(textStyleCss({ bold: false, shadow: false }, "text")).toEqual({ "--style-weight": "normal", "--style-shadow": "none" });
    // The shadow's colour goes into the text's own shadow, drawn when it is on.
    expect(textStyleCss({ shadowColor: "#00FF00" }, "question")).toEqual({ "--style-shadow-color": "rgba(0, 255, 0, 1)" });
  });

  it("aligns a text across the screen and an answer in its panel", () => {
    expect(textStyleCss({ align: "left" }, "text")).toEqual({ "--style-align": "left", "--style-stretch": "stretch" });
    expect(textStyleCss({ align: "right" }, "answer")).toEqual({ "--style-align": "right" });
  });

  it("puts an answer's fill under the theme's sheen and boxes another text's", () => {
    const fill = { from: "#000000", to: "#000000", kind: "horizontal" } as const;
    expect(textStyleCss({ fill, transparency: 50 }, "answer")).toEqual({
      "--answer-fill": "linear-gradient(to right, rgba(0, 0, 0, 0.5), rgba(0, 0, 0, 0.5))",
    });
    // Without a fill of its own, an answer's theme colour shows through.
    expect(textStyleCss({ transparency: 30 }, "answer")).toEqual({ "--answer-fill": "color-mix(in srgb, var(--answer) 70%, transparent)" });
    expect(textStyleCss({ fill }, "title")).toMatchObject({ "--style-fill": "linear-gradient(to right, rgba(0, 0, 0, 1), rgba(0, 0, 0, 1))", "--style-boxed": 1 });
    // A title has no fill to make transparent.
    expect(textStyleCss({ transparency: 30 }, "title")).toEqual({});
  });

  it("knows which texts slide.css draws bold and with a shadow", () => {
    const css = (readFileSync(new URL("../host/slide.css", import.meta.url), "utf8") as string).replace(/\/\*[\s\S]*?\*\//g, "");
    const selectors: Record<StyledText, string> = { title: ".slide-title", question: ".question-text", text: ".slide-text", answer: ".answer" };
    for (const [text, selector] of Object.entries(selectors) as [StyledText, string][]) {
      // The rule in "Styled texts" that gives the text its weight and shadow.
      const rule = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
        ([, head, body]) => head.split(",").some((part: string) => part.trim() === selector) && body.includes("--style-weight"),
      );
      expect(rule, selector).toBeDefined();
      const body = rule?.[2] ?? "";
      expect(body.includes("var(--style-weight, var(--text-bold))"), `${selector} bold`).toBe(THEME_TEXT[text].bold);
      expect(body.includes("var(--style-shadow, var(--text-shadow))"), `${selector} shadow`).toBe(THEME_TEXT[text].shadow);
    }
  });

  it("keeps text readable on a fill of its own without a colour of its own", () => {
    const fill = (from: string, to: string) => ({ from, to, kind: "vertical" }) as const;
    const color = (style: Parameters<typeof textStyleCss>[0], text: StyledText) => (textStyleCss(style, text) as Record<string, unknown>)["--style-color"];
    expect(color({ fill: fill("#FFFFFF", "#FFF4C2") }, "answer")).toBe("#1a1a1a");
    expect(color({ fill: fill("#0B1F4A", "#1E3C72") }, "question")).toBe("#ffffff");
    // A gradient from light to dark: the text that reads better on its worse end.
    expect(color({ fill: fill("#FFFFFF", "#1E3C72") }, "text")).toBe("#1a1a1a");
    // A colour of its own stays, and a fill that mostly shows through leaves the theme's.
    expect(color({ fill: fill("#FFFFFF", "#FFFFFF"), color: "#FF0000" }, "answer")).toBe("rgba(255, 0, 0, 1)");
    expect(color({ fill: fill("#FFFFFF", "#FFFFFF"), transparency: 60 }, "answer")).toBeUndefined();
  });
});
