import { describe, expect, it } from "vitest";
import { newSlide } from "./editor/slides";
import { sameContext, slideContexts } from "./quiz";
import type { Quiz, SlideType } from "./types";

const quizOf = (...types: SlideType[]): Quiz => ({
  schemaVersion: 2,
  title: "",
  slides: types.map((type, i) => newSlide(type, `s${i}`)),
});

describe("slideContexts", () => {
  it("numbers the questions", () => {
    const contexts = slideContexts(quizOf("billboard", "question", "question", "endOfRound"));
    expect(contexts.map((c) => c.position)).toEqual([null, { n: 1, total: 2 }, { n: 2, total: 2 }, null]);
  });

  it("links an explanation to the nearest question before it in its round", () => {
    const quiz = quizOf("explanation", "question", "billboard", "explanation", "question", "explanation", "endOfRound", "explanation");
    const explains = slideContexts(quiz).map((c) => (c.explains ? `${c.explains.question.id} ${c.explains.position.n}` : null));
    expect(explains).toEqual([null, null, null, "s1 1", null, "s4 2", null, null]);
  });
});

describe("sameContext", () => {
  it("compares what the slide shows, not the objects", () => {
    const quiz = quizOf("question", "explanation");
    const [, before] = slideContexts(quiz);
    const [, after] = slideContexts(quiz);
    expect(before).not.toBe(after);
    expect(sameContext(before, after)).toBe(true);
    const edited = slideContexts({ ...quiz, slides: [{ ...quiz.slides[0], notes: "x" }, quiz.slides[1]] });
    expect(sameContext(before, edited[1])).toBe(false);
  });
});
