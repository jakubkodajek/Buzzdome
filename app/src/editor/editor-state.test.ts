import { describe, expect, it } from "vitest";
import type { EditorDraft, Quiz, ValidationError } from "../types";
import { editorReducer, isUnsaved, LOADING, needsInfo, sameData, type EditorState, type OpenQuiz } from "./editor-state";

const quiz: Quiz = { schemaVersion: 1, title: "Quiz", theme: null, slides: [] };
const draft = (unsaved: boolean, dropped: ValidationError[] = []): EditorDraft => ({
  id: "draft-1",
  quiz,
  sourcePath: "/q.quiz",
  savesInPlace: true,
  unsaved,
  mediaRoot: "/d/",
  problems: [],
  dropped,
});

function opened(state: EditorState): OpenQuiz {
  if (state.status !== "open") throw new Error(`expected an open quiz, got ${state.status}`);
  return state;
}

describe("editor state", () => {
  it("knows whether the quiz is saved, through edits and undo", () => {
    let state = editorReducer(LOADING, { type: "loaded", draft: draft(false) });
    expect(isUnsaved(opened(state))).toBe(false);
    state = editorReducer(state, { type: "edit", edit: { type: "setTitle", title: "New" }, at: 0 });
    expect(isUnsaved(opened(state))).toBe(true);
    state = editorReducer(state, { type: "undo" });
    expect(isUnsaved(opened(state))).toBe(false);
    // A draft left unsaved last time stays unsaved until it is saved.
    state = editorReducer(state, { type: "loaded", draft: draft(true) });
    expect(isUnsaved(opened(state))).toBe(true);
    state = editorReducer(state, { type: "saved", quiz: opened(state).history.present, path: "/other.quiz" });
    expect(opened(state)).toMatchObject({ sourcePath: "/other.quiz" });
    expect(isUnsaved(opened(state))).toBe(false);
  });

  it("takes the backend's problems only for the quiz as it is now", () => {
    let state = editorReducer(LOADING, { type: "loaded", draft: draft(false) });
    const before = opened(state).history.present;
    state = editorReducer(state, { type: "edit", edit: { type: "setTitle", title: "New" }, at: 0 });
    const problem: ValidationError[] = [{ slideId: null, problem: { code: "duplicateSlideId" } }];
    expect(editorReducer(state, { type: "checked", quiz: before, problems: problem })).toBe(state);
    state = editorReducer(state, { type: "checked", quiz: opened(state).history.present, problems: problem });
    expect(opened(state).problems).toEqual(problem);
  });

  it("tells what opening left out until the quiz is saved without it", () => {
    const dropped: ValidationError[] = [{ slideId: "e", problem: { code: "mediaOnRanking" } }];
    let state = editorReducer(LOADING, { type: "loaded", draft: draft(true, dropped) });
    expect(opened(state).dropped).toEqual(dropped);
    expect(needsInfo(opened(state))).toBe(true);
    state = editorReducer(state, { type: "edit", edit: { type: "setTitle", title: "New" }, at: 0 });
    expect(opened(state).dropped).toEqual(dropped);
    state = editorReducer(state, { type: "saved", quiz: opened(state).history.present, path: "/q.quiz" });
    expect(opened(state).dropped).toEqual([]);
    expect(needsInfo(opened(state))).toBe(false);
  });

  it("goes on with undo where it left the draft, while the draft holds the same quiz", () => {
    let left = editorReducer(LOADING, { type: "loaded", draft: draft(false) });
    left = editorReducer(left, { type: "edit", edit: { type: "setTitle", title: "New" }, at: 0 });
    const kept = opened(left);
    // The backend gives the quiz back as JSON: another object, with nulls for what was left out.
    const back = { ...draft(true), quiz: { ...JSON.parse(JSON.stringify(kept.history.present)), theme: null }, problems: [{ slideId: null, problem: { code: "duplicateSlideId" } }] } as EditorDraft;
    let state = editorReducer(LOADING, { type: "loaded", draft: back, kept });
    expect(opened(state).history).toBe(kept.history);
    expect(opened(state).problems).toEqual(back.problems);
    expect(isUnsaved(opened(state))).toBe(true);
    state = editorReducer(state, { type: "undo" });
    expect(isUnsaved(opened(state))).toBe(false);
    // Another draft, or the same one changed meanwhile, starts afresh.
    expect(opened(editorReducer(LOADING, { type: "loaded", draft: { ...back, id: "draft-2" }, kept })).history.past).toEqual([]);
    expect(opened(editorReducer(LOADING, { type: "loaded", draft: { ...back, quiz: { ...back.quiz, title: "Other" } }, kept })).history.past).toEqual([]);
  });

  it("compares data as JSON carries it", () => {
    expect(sameData({ a: 1, b: [1, { c: null }] }, { b: [1, {}], a: 1 })).toBe(true);
    expect(sameData({ a: undefined }, { a: null })).toBe(true);
    expect(sameData({ a: 1 }, { a: 2 })).toBe(false);
    expect(sameData([1, 2], [1])).toBe(false);
    expect(sameData({ a: [] }, { a: {} })).toBe(false);
    expect(sameData(0, null)).toBe(false);
  });

  it("has nothing open without a draft", () => {
    expect(editorReducer(LOADING, { type: "loaded", draft: null })).toEqual({ status: "empty" });
    expect(editorReducer({ status: "empty" }, { type: "undo" })).toEqual({ status: "empty" });
  });

  it("keeps a draft that cannot be read apart from having none, until it reads", () => {
    const error = { code: "draftNotRead", detail: "line 1" };
    const state = editorReducer(LOADING, { type: "unreadable", error });
    expect(state).toEqual({ status: "unreadable", error });
    expect(editorReducer(state, { type: "undo" })).toBe(state);
    expect(editorReducer(state, { type: "loading" })).toBe(LOADING);
    expect(opened(editorReducer(LOADING, { type: "loaded", draft: draft(false) })).draftId).toBe("draft-1");
  });
});
