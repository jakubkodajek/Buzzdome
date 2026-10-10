import { describe, expect, it } from "vitest";
import type { Quiz } from "../types";
import { historyReducer, MERGE_MS, startHistory, UNDO_LIMIT, type History } from "./history";
import { newSlide } from "./slides";

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
