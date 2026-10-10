import { describe, expect, it } from "vitest";
import { slideTitle } from "../quiz";
import type { Category, Quiz } from "../types";
import { freshCategoryId, newCategory, sampleSlide, slidesOf, withLook } from "./categories";
import { newSlide, setBackground, setSlideFill, setTextStyle } from "./slides";

const films: Category = { id: "c-1", name: "Films", background: null, style: { headline: { bold: true } } };
const quiz: Quiz = {
  schemaVersion: 1,
  title: "",
  theme: null,
  slides: [{ ...newSlide("question", "a"), category: "c-1" }, newSlide("billboard", "b"), { ...newSlide("final", "c"), category: "c-1" }],
  categories: [films],
};

describe("categories", () => {
  it("get ids no category has", () => {
    const ids = ["1", "2"];
    expect(freshCategoryId(quiz, () => ids.shift() as string)).toBe("c-2");
  });

  it("start with the theme's look or the look of a slide", () => {
    expect(newCategory("c-9", "Music")).toEqual({ id: "c-9", name: "Music", background: null, style: undefined });
    const slide = setTextStyle(setBackground(newSlide("billboard", "b"), "media/stage.jpg"), "text", { italic: true });
    expect(newCategory("c-9", "Music", slide)).toEqual({ id: "c-9", name: "Music", background: slide.background, style: { text: { italic: true } } });
  });

  it("are drawn as a question asking their name, the same slide while they stay the same", () => {
    const sample = sampleSlide(films);
    expect(sample).toMatchObject({ type: "question", style: films.style, background: null });
    expect(slideTitle(sample)).toBe("Films");
    expect(sampleSlide(films)).toBe(sample);
    expect(sampleSlide({ ...films })).not.toBe(sample);
  });

  it("take the look of their sample slide as it is changed", () => {
    const sample = sampleSlide(films);
    expect(withLook(films, sample)).toBe(films);
    // What is no look, such as the text, is not the category's.
    expect(withLook(films, { ...sample, notes: "x" })).toBe(films);
    const red = "#c80000";
    const changed = setBackground(setSlideFill(sample, { from: red, to: red, kind: "radial" }), "media/loop.mp4");
    expect(withLook(films, changed)).toEqual({ ...films, background: changed.background, style: { ...films.style, fill: { from: red, to: red, kind: "radial" } } });
  });

  it("know the slides that took them on", () => {
    expect(slidesOf(quiz, "c-1")).toEqual(["a", "c"]);
    expect(slidesOf(quiz, "c-2")).toEqual([]);
  });
});
