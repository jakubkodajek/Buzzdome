import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { SessionSummary } from "../types";
import { startQuestion, staleFile, staleFor, type DraftState } from "./show-start";

const t = translator("en");
const last: SessionSummary = { meta: { quizTitle: "Pub quiz", sourcePath: null, startedMs: 0, mediaRoot: null, rulesVersion: 1 }, commandCount: 57 };
const edited: DraftState = { title: "Edited", sourcePath: "/quizzes/a.quiz", unsaved: true };

describe("startQuestion", () => {
  const replaces = t("replaceLastShowConfirm", { title: "Pub quiz" });
  const stale = t("startStaleConfirm", { title: "Edited" });

  it("asks nothing with no show to resume and no newer draft", () => {
    expect(startQuestion(t, null, null, "/quizzes/a.quiz")).toBeNull();
    expect(startQuestion(t, null, { ...edited, unsaved: false }, "/quizzes/a.quiz")).toBeNull();
  });

  it("asks before a new show ends the way back to the last one, whatever it plays, naming the step on its button", () => {
    expect(startQuestion(t, last, null)).toEqual({ text: replaces, ok: t("startNewShow") });
    expect(startQuestion(t, last, null, "/quizzes/b.quiz")).toEqual({ text: replaces, ok: t("startNewShow") });
  });

  it("asks nothing of a last show that was finished, which a new one loses nothing of", () => {
    expect(startQuestion(t, { ...last, finished: "ended" }, null)).toBeNull();
    expect(startQuestion(t, { ...last, finished: "playedOut" }, null, "/quizzes/b.quiz")).toBeNull();
    expect(startQuestion(t, { ...last, finished: "ended" }, edited, "/quizzes/a.quiz")).toEqual({
      text: t("startStaleConfirm", { title: "Edited" }),
      ok: t("startSavedVersion"),
    });
  });

  it("asks before playing the file of a draft with unsaved changes, in the same question as the one about the last show", () => {
    expect(startQuestion(t, null, edited, "/quizzes/a.quiz")).toEqual({ text: stale, ok: t("startSavedVersion") });
    expect(startQuestion(t, last, edited, "/quizzes/a.quiz")).toEqual({ text: `${replaces}\n\n${stale}`, ok: t("startNewShow") });
    expect(startQuestion(t, null, edited, "/quizzes/b.quiz")).toBeNull();
    // A file not known before the start (the picker's): the Director says so once it opened.
    expect(startQuestion(t, null, edited)).toBeNull();
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

describe("staleFor", () => {
  it("is such a draft only for its own file", () => {
    expect(staleFor(edited, "/quizzes/a.quiz")).toBe(edited);
    expect(staleFor(edited, "/quizzes/b.quiz")).toBeNull();
    expect(staleFor({ ...edited, unsaved: false }, "/quizzes/a.quiz")).toBeNull();
    expect(staleFor(null, "/quizzes/a.quiz")).toBeNull();
  });
});
