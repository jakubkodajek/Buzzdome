import { describe, expect, it } from "vitest";
import type { Category, Quiz } from "../types";
import { applyEdit, historyReducer, MERGE_MS, startHistory, UNDO_LIMIT, type History } from "./history";
import { newSlide, setTextStyle } from "./slides";

const quiz: Quiz = {
  schemaVersion: 1,
  title: "Quiz",
  theme: "builtin.night",
  slides: [newSlide("billboard", "a"), newSlide("question", "b"), newSlide("endOfRound", "c")],
};
const ids = (h: History) => h.present.slides.map((s) => s.id);

describe("history", () => {
  it("undoes and redoes structural changes", () => {
    let h = startHistory(quiz);
    h = historyReducer(h, { type: "edit", edit: { type: "moveSlide", from: 0, to: 2 }, at: 0 });
    expect(ids(h)).toEqual(["b", "c", "a"]);
    h = historyReducer(h, { type: "edit", edit: { type: "removeSlide", index: 1 }, at: 1 });
    expect(ids(h)).toEqual(["b", "a"]);
    h = historyReducer(h, { type: "undo" });
    expect(ids(h)).toEqual(["b", "c", "a"]);
    h = historyReducer(h, { type: "undo" });
    expect(h.present).toBe(quiz);
    h = historyReducer(h, { type: "redo" });
    expect(ids(h)).toEqual(["b", "c", "a"]);
  });

  it("joins quick typing in one field into one undo step", () => {
    let h = startHistory(quiz);
    for (const [title, at] of [["Q", 0], ["Qu", 100], ["Qui", 200]] as const) {
      h = historyReducer(h, { type: "edit", edit: { type: "setTitle", title }, merge: "title", at });
    }
    expect(h.past).toHaveLength(1);
    // A pause, or another field, starts a new step.
    h = historyReducer(h, { type: "edit", edit: { type: "setTitle", title: "Quiz!" }, merge: "title", at: 200 + MERGE_MS });
    expect(h.past).toHaveLength(2);
    h = historyReducer(h, { type: "undo" });
    h = historyReducer(h, { type: "undo" });
    expect(h.present.title).toBe("Quiz");
  });

  it("ignores changes that change nothing and drops redo after a change", () => {
    const h = startHistory(quiz);
    expect(historyReducer(h, { type: "edit", edit: { type: "removeSlide", index: 9 }, at: 0 })).toBe(h);
    expect(historyReducer(h, { type: "edit", edit: { type: "setTitle", title: "Quiz" }, at: 0 })).toBe(h);
    expect(historyReducer(h, { type: "edit", edit: { type: "setTheme", theme: "builtin.night" }, at: 0 })).toBe(h);
    expect(historyReducer(h, { type: "undo" })).toBe(h);
    expect(historyReducer(h, { type: "edit", edit: { type: "updateSlide", id: "a", update: (s) => s }, at: 0 })).toBe(h);
    expect(historyReducer(h, { type: "edit", edit: { type: "updateSlide", id: "gone", update: (s) => ({ ...s, notes: "x" }) }, at: 0 })).toBe(h);
    let changed = historyReducer(h, { type: "edit", edit: { type: "setTitle", title: "A" }, at: 0 });
    changed = historyReducer(changed, { type: "undo" });
    changed = historyReducer(changed, { type: "edit", edit: { type: "setTitle", title: "B" }, at: 1 });
    expect(changed.future).toEqual([]);
  });

  it("changes the theme as one undo step", () => {
    let h = startHistory(quiz);
    h = historyReducer(h, { type: "edit", edit: { type: "setTheme", theme: "builtin.neon" }, at: 0 });
    expect(h.present.theme).toBe("builtin.neon");
    h = historyReducer(h, { type: "undo" });
    expect(h.present.theme).toBe("builtin.night");
  });

  it("applies a batch of edits as one undo step", () => {
    const rename = (id: string, notes: string) => ({ type: "updateSlide" as const, id, update: (s: (typeof quiz.slides)[number]) => (s.notes === notes ? s : { ...s, notes }) });
    let h = startHistory(quiz);
    h = historyReducer(h, { type: "edit", edit: { type: "batch", edits: [rename("a", "x"), rename("c", "y")] }, at: 0 });
    expect(h.present.slides.map((s) => s.notes)).toEqual(["x", "", "y"]);
    expect(h.past).toHaveLength(1);
    // A batch that changes nothing is no step at all.
    const same = historyReducer(h, { type: "edit", edit: { type: "batch", edits: [rename("a", "x"), rename("gone", "z")] }, at: 1 });
    expect(same).toBe(h);
    h = historyReducer(h, { type: "undo" });
    expect(h.present).toBe(quiz);
  });

  it("updates a slide as it is when the update lands", () => {
    let h = startHistory(quiz);
    h = historyReducer(h, { type: "edit", edit: { type: "moveSlide", from: 0, to: 2 }, at: 0 });
    h = historyReducer(h, { type: "edit", edit: { type: "updateSlide", id: "a", update: (s) => ({ ...s, notes: "moved" }) }, at: 1 });
    expect(h.present.slides[2]).toMatchObject({ id: "a", notes: "moved" });
  });

  it("inserts at the end and keeps a bounded number of undo steps", () => {
    let h = startHistory(quiz);
    for (let i = 0; i < UNDO_LIMIT + 10; i++) {
      h = historyReducer(h, { type: "edit", edit: { type: "insertSlide", index: h.present.slides.length, slide: newSlide("billboard", `n${i}`) }, at: i });
    }
    expect(h.present.slides).toHaveLength(3 + UNDO_LIMIT + 10);
    expect(h.past).toHaveLength(UNDO_LIMIT);
  });
});

describe("category edits", () => {
  const films: Category = { id: "c-1", name: "Films", background: null, style: { headline: { bold: true } } };
  const music: Category = { id: "c-2", name: "Music", background: null };
  const withCategories = applyEdit(applyEdit(quiz, { type: "insertCategory", index: 0, category: films }), { type: "insertCategory", index: 1, category: music });

  it("inserts, updates and removes categories", () => {
    expect(withCategories.categories).toEqual([films, music]);
    expect(applyEdit(quiz, { type: "insertCategory", index: 1, category: films })).toBe(quiz);
    // A twin of a category there (a double click) is not inserted.
    expect(applyEdit(withCategories, { type: "insertCategory", index: 2, category: { ...films, name: "Films 2" } })).toBe(withCategories);
    const renamed = applyEdit(withCategories, { type: "updateCategory", id: "c-2", update: (c) => ({ ...c, name: "Songs" }) });
    expect(renamed.categories?.map((c) => c.name)).toEqual(["Films", "Songs"]);
    expect(applyEdit(withCategories, { type: "updateCategory", id: "c-2", update: (c) => c })).toBe(withCategories);
    expect(applyEdit(withCategories, { type: "updateCategory", id: "gone", update: (c) => ({ ...c, name: "x" }) })).toBe(withCategories);
    expect(applyEdit(withCategories, { type: "removeCategory", id: "gone" })).toBe(withCategories);
  });

  it("gives slides a category's look as it is when the edit lands, and unlinks them from one removed", () => {
    const bolder = applyEdit(withCategories, { type: "updateCategory", id: "c-1", update: (c) => ({ ...c, style: { headline: { bold: true, size: 90 } } }) });
    const applied = applyEdit(bolder, { type: "applyCategory", id: "c-1", slides: ["b", "c"] });
    expect(applied.slides.map((s) => s.category)).toEqual([undefined, "c-1", "c-1"]);
    expect(applied.slides[1].style).toEqual({ headline: { bold: true, size: 90 } });
    expect(applyEdit(bolder, { type: "applyCategory", id: "gone", slides: ["b"] })).toBe(bolder);
    expect(applyEdit(bolder, { type: "applyCategory", id: "c-1", slides: ["gone"] })).toBe(bolder);

    const restyled = applyEdit(applied, { type: "updateSlide", id: "c", update: (s) => setTextStyle(s, "headline", { italic: true }) });
    const removed = applyEdit(restyled, { type: "removeCategory", id: "c-1" });
    expect(removed.categories).toEqual([music]);
    expect(removed.slides.map((s) => s.category)).toEqual([undefined, undefined, undefined]);
    // Their look stays.
    expect(removed.slides[2].style).toEqual(restyled.slides[2].style);
  });
});
