import { describe, expect, it } from "vitest";
import { DEFAULT_QUIZ_SETTINGS } from "../bindings/Constants";
import type { Quiz, QuizChange, Slide } from "../types";
import { changeCount, changeEdits } from "./ai-changes";
import { newCategory } from "./categories";
import { applyEdit, historyReducer, startHistory } from "./history";
import { newSlide } from "./slides";

const quiz: Quiz = {
  schemaVersion: 1,
  title: "Kvíz",
  theme: "builtin.night",
  slides: [newSlide("billboard", "a"), newSlide("question", "b"), newSlide("endOfRound", "c")],
};
const ids = (q: Quiz) => q.slides.map((s) => s.id);
let n = 0;
const random = () => `n${++n}`;
const made = (q: Quiz, changes: QuizChange[]) => {
  const result = changeEdits(q, changes, random);
  return { ...result, quiz: applyEdit(q, result.edit) };
};

describe("the AI assistant's changes", () => {
  it("are made in order by the slides' ids, as one undo step", () => {
    const edited: Slide = { ...quiz.slides[1], notes: "nové" };
    const changes: QuizChange[] = [
      { type: "replaceSlide", slide: edited },
      { type: "insertSlides", after: "c", slides: [newSlide("final", "")] },
      { type: "moveSlide", id: "c", after: "a" },
      { type: "removeSlide", id: "a" },
      { type: "setTitle", title: "Města" },
    ];
    const result = made(quiz, changes);
    expect(ids(result.quiz)).toEqual(["c", "b", result.added[0]]);
    expect(result.quiz.slides[1].notes).toBe("nové");
    expect(result.quiz.title).toBe("Města");
    expect(result.counts).toEqual({ added: 1, changed: 1, removed: 1, moved: 1, quiz: 1 });
    expect(changeCount(result.counts)).toBe(5);
    let history = startHistory(quiz);
    history = historyReducer(history, { type: "edit", edit: result.edit, at: 0 });
    history = historyReducer(history, { type: "undo" });
    expect(history.present).toBe(quiz);
  });

  it("put added slides after the slide named, or at the start, with ids of their own", () => {
    const two = [newSlide("billboard", "b"), newSlide("question", "b")];
    const result = made(quiz, [{ type: "insertSlides", after: "a", slides: two }, { type: "insertSlides", after: null, slides: [newSlide("billboard", "")] }]);
    expect(result.added).toHaveLength(3);
    expect(new Set(ids(result.quiz)).size).toBe(6);
    expect(ids(result.quiz)).toEqual([result.added[2], "a", result.added[0], result.added[1], "b", "c"]);
  });

  it("move a slide down or up to just after the slide named", () => {
    expect(ids(made(quiz, [{ type: "moveSlide", id: "a", after: "c" }]).quiz)).toEqual(["b", "c", "a"]);
    expect(ids(made(quiz, [{ type: "moveSlide", id: "c", after: null }]).quiz)).toEqual(["c", "a", "b"]);
    expect(ids(made(quiz, [{ type: "moveSlide", id: "a", after: "b" }]).quiz)).toEqual(["b", "a", "c"]);
  });

  it("do nothing to a slide that is gone, and count only what changed", () => {
    const result = made(quiz, [
      { type: "removeSlide", id: "x" },
      { type: "replaceSlide", slide: newSlide("billboard", "x") },
      { type: "moveSlide", id: "x", after: null },
      { type: "replaceSlide", slide: quiz.slides[0] },
      { type: "setSettings", settings: DEFAULT_QUIZ_SETTINGS },
    ]);
    expect(result.edit).toEqual({ type: "batch", edits: [] });
    expect(changeCount(result.counts)).toBe(0);
  });

  it("change the categories first, so new slides take on a new category's look", () => {
    const films = { ...newCategory("c-1", "Filmy"), background: { file: "media/film.jpg" } } as ReturnType<typeof newCategory>;
    const old = newCategory("c-2", "Staré");
    const withOld: Quiz = { ...quiz, categories: [old] };
    const slide: Slide = { ...newSlide("question", ""), category: "c-1" };
    const result = made(withOld, [
      { type: "insertSlides", after: "c", slides: [slide] },
      { type: "setCategories", categories: [films] },
    ]);
    expect(result.quiz.categories?.map((c) => c.id)).toEqual(["c-1"]);
    const added = result.quiz.slides.find((s) => s.id === result.added[0]);
    expect(added?.category).toBe("c-1");
    expect(added?.background).toEqual(films.background);
    expect(result.counts.quiz).toBe(1);
  });
});
