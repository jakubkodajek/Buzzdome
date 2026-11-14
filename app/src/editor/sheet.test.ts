import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { Quiz } from "../types";
import { exampleSheet, listSeparator, templateQuiz } from "./sheet";

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
    expect(template.slides[1]).toMatchObject({ mode: "everyone", answers: ["Paris", "Lyon", "Marseille", "Nice"], correct: [{ answer: 0, share: 100 }] });
    expect(template.slides[2]).toMatchObject({ mode: "fastestFinger", answers: [], correct: [] });
    expect(template.slides.some((slide) => slide.category !== undefined)).toBe(false);
  });

  it("keep the template to the basic columns, also for a quiz with categories", () => {
    const quiz: Quiz = { schemaVersion: 5, title: "", theme: null, slides: [], categories: [{ id: "c-1", name: "Films", background: null }] };
    expect(templateQuiz(translator("en"), quiz).slides.some((slide) => slide.category !== undefined)).toBe(false);
  });

  it("show an example under the headings and types the import reads, in the quizmaster's language", () => {
    expect(exampleSheet(translator("cs"), "cs")).toEqual({
      headings: ["Typ", "Otázka", "Odpověď 1", "Odpověď 2", "Odpověď 3", "Odpověď 4", "Poznámky"],
      rows: [
        ["Sekce", "Kolo 1", "", "", "", "", ""],
        ["", "Hlavní město Francie?", "[Paříž]", "Lyon", "Marseille", "Nice", ""],
        ["", "Kolik obyvatel má Paříž?", "", "", "", "", "Odpovídá se nahlas"],
        ["Konec kola", "Konec kola 1", "", "", "", "", ""],
      ],
    });
    const en = exampleSheet(translator("en"), "en");
    expect([en.headings[0], en.rows[0][0], en.rows[3][0]]).toEqual(["Type", "Section", "End of round"]);
  });
});
