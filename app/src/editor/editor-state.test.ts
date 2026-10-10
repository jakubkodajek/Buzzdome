import { describe, expect, it } from "vitest";
import type { EditorDraft, Quiz, ValidationError } from "../types";
import { editorReducer, isUnsaved, LOADING, needsInfo, type EditorState, type OpenQuiz } from "./editor-state";

const quiz: Quiz = { schemaVersion: 1, title: "Quiz", theme: null, slides: [] };
const draft = (unsaved: boolean, dropped: ValidationError[] = []): EditorDraft => ({
  quiz,
  sourcePath: "/q.quiz",
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

  it("has nothing open without a draft", () => {
    expect(editorReducer(LOADING, { type: "loaded", draft: null })).toEqual({ status: "empty" });
    expect(editorReducer({ status: "empty" }, { type: "undo" })).toEqual({ status: "empty" });
  });
});
