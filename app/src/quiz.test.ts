import { describe, expect, it } from "vitest";
import { FULL_SHARE } from "./bindings/Constants";
import { newSlide } from "./editor/slides";
import { hasTitle, isCorrect, plays, rightAnswers, sameContext, shareOf, slideContexts, slideTitle } from "./quiz";
import type { Quiz, SlideType } from "./types";

const quizOf = (...types: SlideType[]): Quiz => ({
  schemaVersion: 2,
  title: "",
  theme: null,
  slides: types.map((type, i) => newSlide(type, `s${i}`)),
});

describe("slideContexts", () => {
  it("numbers the questions", () => {
    const contexts = slideContexts(quizOf("billboard", "question", "question", "endOfRound"));
    expect(contexts.map((c) => c.position)).toEqual([null, { n: 1, total: 2 }, { n: 2, total: 2 }, null]);
  });

  it("links an explanation to the nearest question before it in its round", () => {
    const quiz = quizOf("explanation", "question", "billboard", "explanation", "question", "explanation", "endOfRound", "explanation", "question", "final", "explanation");
    const explains = slideContexts(quiz).map((c) => (c.explains ? `${c.explains.question.id} ${c.explains.position.n}` : null));
    expect(explains).toEqual([null, null, null, "s1 1", null, "s4 2", null, null, null, null, null]);
  });
});

describe("slide kinds", () => {
  it("lists billboards, round ends and final slides by their title", () => {
    const final = { ...newSlide("final", "f"), title: "Winners" };
    expect(hasTitle(final)).toBe(true);
    expect(slideTitle(final)).toBe("Winners");
    expect(hasTitle(newSlide("explanation", "e"))).toBe(false);
    expect(["endOfRound", "final"].some((type) => plays(type as SlideType).pictureAndVideo)).toBe(false);
    expect(plays("billboard")).toEqual({ pictureAndVideo: true, clockAndReveal: false });
    expect(plays("question").clockAndReveal).toBe(true);
  });
});

describe("shareOf", () => {
  it("gives a right answer's share, and all the points on a fastest-finger question", () => {
    const question = { mode: "everyone" as const, correct: [{ answer: 1, share: 50 }] };
    expect([shareOf(question, 0), shareOf(question, 1), isCorrect(question, 1)]).toEqual([null, 50, true]);
    expect(shareOf({ ...question, mode: "fastestFinger" }, 1)).toBe(FULL_SHARE);
  });
});

describe("rightAnswers", () => {
  it("lists the right answers the question has in their order, with the share each gives", () => {
    const question = {
      mode: "everyone" as const,
      answers: ["a", "b", "c"],
      correct: [
        { answer: 2, share: FULL_SHARE },
        { answer: 0, share: 50 },
        { answer: 3, share: FULL_SHARE },
      ],
    };
    expect(rightAnswers(question)).toEqual([
      { answer: 0, share: 50 },
      { answer: 2, share: FULL_SHARE },
    ]);
    expect(rightAnswers({ ...question, mode: "fastestFinger" }).map((r) => r.share)).toEqual([FULL_SHARE, FULL_SHARE]);
    expect(rightAnswers({ ...question, correct: [] })).toEqual([]);
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
