import { describe, expect, it } from "vitest";
// @ts-expect-error type error without @types/node package
import { readFileSync } from "node:fs";
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
const made = (q: Quiz, changes: QuizChange[], asked: Quiz = q) => {
  const result = changeEdits(q, asked, changes, random);
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
    expect(result.counts).toEqual({ added: 1, changed: 1, removed: 1, moved: 1, quiz: 1, skipped: 0 });
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

  it("place later changes after slides the same reply added, by the reply's ids", () => {
    const x: Slide = { ...newSlide("billboard", ""), id: "ai-new-1" };
    const y: Slide = { ...newSlide("billboard", ""), id: "ai-new-2" };
    const result = made(quiz, [
      { type: "insertSlides", after: "a", slides: [x] },
      { type: "insertSlides", after: "ai-new-1", slides: [y] },
      { type: "moveSlide", id: "c", after: "ai-new-1" },
    ]);
    expect(ids(result.quiz)).toEqual(["a", result.added[0], "c", result.added[1], "b"]);
    expect(result.added.some((id) => id.startsWith("ai-new-"))).toBe(false);
  });

  it("keep what the quizmaster changed while the model wrote", () => {
    const typed: Quiz = { ...quiz, title: "Můj název", slides: quiz.slides.map((s) => (s.id === "b" ? { ...s, notes: "moje" } : s)) };
    const result = made(typed, [
      { type: "replaceSlide", slide: { ...quiz.slides[1], notes: "modelu" } },
      { type: "replaceSlide", slide: { ...quiz.slides[0], notes: "modelu" } },
      { type: "setTitle", title: "Model" },
      { type: "removeSlide", id: "b" },
    ], quiz);
    expect(result.quiz.title).toBe("Můj název");
    expect(result.quiz.slides[1].notes).toBe("moje");
    expect(result.quiz.slides[0].notes).toBe("modelu");
    expect(result.counts).toMatchObject({ changed: 1, quiz: 0, removed: 0, skipped: 3 });
  });

  it("take slides as the backend made them, with their category's look and the style written over it", () => {
    const films = { ...newCategory("c-1", "Filmy"), background: { file: "media/film.jpg" } } as ReturnType<typeof newCategory>;
    const withOld: Quiz = { ...quiz, categories: [newCategory("c-2", "Staré")] };
    const styled: Slide = { ...withOld.slides[0], category: "c-1", style: { headline: { color: "#FF0000" } } };
    const result = made(withOld, [
      { type: "setCategories", categories: [films] },
      { type: "replaceSlide", slide: styled },
    ]);
    expect(result.quiz.categories?.map((c) => c.id)).toEqual(["c-1"]);
    expect(result.quiz.slides[0]).toEqual(styled);
    expect(result.counts).toMatchObject({ changed: 1, quiz: 1 });
  });

  // The same cases `quiz_edits.rs` makes for the MCP server: the two must make them alike.
  const fixture = JSON.parse(readFileSync(new URL("../../../crates/quiz-ai/tests/fixtures/quiz-changes.json", import.meta.url), "utf8") as string) as {
    cases: { name: string; quiz: Quiz; changes: QuizChange[]; expected: Quiz }[];
  };
  for (const { name, quiz: before, changes, expected } of fixture.cases) {
    it(`make the shared case: ${name}`, () => {
      const { quiz: after } = made(before, changes);
      const had = new Set(before.slides.map((s) => s.id));
      let added = 0;
      const named = { ...after, slides: after.slides.map((s) => (had.has(s.id) ? s : { ...s, id: `new-${++added}` })) };
      expect(JSON.parse(JSON.stringify(named))).toEqual(expected);
    });
  }
});
