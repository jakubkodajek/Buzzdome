import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { SessionSummary } from "../types";
import { startQuestions, staleFile, type DraftState } from "./show-start";

const t = translator("en");
const last: SessionSummary = { meta: { quizTitle: "Pub quiz", sourcePath: null, startedMs: 0, mediaRoot: null, rulesVersion: 1 }, commandCount: 57 };
const edited: DraftState = { title: "Edited", sourcePath: "/quizzes/a.quiz", unsaved: true };

describe("startQuestions", () => {
  it("asks nothing with no show to resume and no newer draft", () => {
    expect(startQuestions(t, null, null, "/quizzes/a.quiz")).toEqual([]);
    expect(startQuestions(t, null, { ...edited, unsaved: false }, "/quizzes/a.quiz")).toEqual([]);
  });

  it("asks before a new show ends the way back to the last one, whatever it plays", () => {
    const replaces = t("replaceLastShowConfirm", { title: "Pub quiz" });
    expect(startQuestions(t, last, null)).toEqual([replaces]);
    expect(startQuestions(t, last, null, "/quizzes/b.quiz")).toEqual([replaces]);
  });

  it("asks before playing the file of a draft with unsaved changes, after the question about the last show", () => {
    const stale = t("startStaleConfirm", { title: "Edited" });
    expect(startQuestions(t, null, edited, "/quizzes/a.quiz")).toEqual([stale]);
    expect(startQuestions(t, last, edited, "/quizzes/a.quiz")).toEqual([t("replaceLastShowConfirm", { title: "Pub quiz" }), stale]);
    expect(startQuestions(t, null, edited, "/quizzes/b.quiz")).toEqual([]);
    // A file not known before the start (the picker's): the notice in the start pane says it instead.
    expect(startQuestions(t, null, edited)).toEqual([]);
  });
});

describe("staleFile", () => {
  it("is a draft with unsaved changes of a quiz file", () => {
    expect(staleFile(edited)).toBe(edited);
    expect(staleFile({ ...edited, unsaved: false })).toBeNull();
    expect(staleFile({ ...edited, sourcePath: null })).toBeNull();
    expect(staleFile(null)).toBeNull();
  });
});
