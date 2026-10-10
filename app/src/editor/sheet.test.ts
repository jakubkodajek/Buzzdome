import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { Quiz } from "../types";
import { listSeparator, templateQuiz } from "./sheet";

describe("spreadsheets", () => {
  it("separate CSV cells as Excel in the quizmaster's language expects", () => {
    expect(listSeparator("cs")).toBe("semicolon");
    expect(listSeparator("en")).toBe("comma");
  });

  it("make the template from sample slides of every kind, in place of the quiz's", () => {
    const quiz: Quiz = { schemaVersion: 4, title: "Mine", theme: "builtin.night", slides: [] };
    const template = templateQuiz(translator("en"), quiz);
    expect({ ...template, slides: [] }).toEqual(quiz);
    expect(template.slides.map((slide) => slide.type)).toEqual(["billboard", "question", "question", "explanation", "endOfRound"]);
    expect(new Set(template.slides.map((slide) => slide.id)).size).toBe(template.slides.length);
    expect(template.slides[1]).toMatchObject({ mode: "everyone", answers: ["Paris", "Lyon", "Marseille", "Nice"], correct: 0 });
    expect(template.slides[2]).toMatchObject({ mode: "fastestFinger", answers: [], correct: null });
  });
});
