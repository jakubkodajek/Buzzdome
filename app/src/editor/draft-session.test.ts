import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EditorDraft, Quiz } from "../types";
import { BUSY, DraftSession, type DraftApi } from "./draft-session";

const quiz = (title: string): Quiz => ({ schemaVersion: 1, title, theme: null, slides: [] });
const draft = (id: string, title = id): EditorDraft => ({ id, quiz: quiz(title), sourcePath: null, unsaved: false, mediaRoot: "/d/", problems: [], dropped: [] });
const FULL_DISK = { code: "file", action: "write", path: "/d/quiz.json", detail: "No space left on device" };

/** A promise with its resolver, to finish a backend call when a test says so. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

const PICKER = { filter: "Quiz files" };

/** A stand-in for the backend: one current draft, and a record of every call. */
function fakeApi(current = "draft-1") {
  const calls: string[] = [];
  const draftApi = {
    editorLoad: vi.fn(async () => draft(current)),
    editorNew: vi.fn(async () => draft((current = "draft-new"))),
    editorOpen: vi.fn(async (path: string | null) => draft((current = `draft-${path}`))),
    editorClose: vi.fn(async () => undefined),
    editorUpdate: vi.fn(async (id: string, written: Quiz) => {
      calls.push(`update ${id} ${written.title}`);
      if (id !== current) throw { code: "draftChanged" };
      return [];
    }),
    editorSave: vi.fn(async (id: string, saved: Quiz, saveAs: boolean) => {
      calls.push(`save ${id} ${saved.title}${saveAs ? " as" : ""}`);
      if (id !== current) throw { code: "draftChanged" };
      return "/q.quiz";
    }),
    editorAddMedia: vi.fn(async () => "media/a.png"),
    editorImportSheet: vi.fn(),
    editorExportSheet: vi.fn(),
    editorTestRun: vi.fn(async () => undefined),
  } satisfies DraftApi;
  return { draftApi, calls };
}

describe("DraftSession", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("starts no file operation while another runs, so Ctrl+S cannot save the old quiz into the one opening", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi);
    const opening = deferred<EditorDraft>();
    draftApi.editorOpen.mockImplementationOnce(() => opening.promise);
    const open = session.exclusive("open", (api) => api.editorOpen("big", PICKER));
    expect(session.getStatus().busy).toBe("open");
    expect(await session.exclusive("save", (api) => api.editorSave("draft-1", quiz("Old"), false, PICKER))).toBe(BUSY);
    opening.resolve(draft("draft-big"));
    expect(await open).toEqual(draft("draft-big"));
    expect(session.getStatus().busy).toBe(null);
    expect(calls).toEqual([]);
  });

  it("writes before a file operation given after it, and the operation's failure is the caller's", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi);
    session.write("draft-1", quiz("A"), true, () => undefined);
    session.write("draft-1", quiz("B"), true, () => undefined);
    await session.exclusive("save", (api) => api.editorSave("draft-1", quiz("B"), true, PICKER));
    // Writes still waiting collapse into the latest.
    expect(calls).toEqual(["update draft-1 B", "save draft-1 B as"]);
    await expect(session.exclusive("save", async () => Promise.reject(FULL_DISK))).rejects.toBe(FULL_DISK);
    expect(session.getStatus().busy).toBe(null);
  });

  it("tries a change that could not be written again until it is, and says so meanwhile", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK).mockRejectedValueOnce(FULL_DISK);
    const checked = vi.fn();
    session.write("draft-1", quiz("A"), true, checked);
    await vi.advanceTimersByTimeAsync(0);
    expect(session.getStatus().notWritten).toBe(FULL_DISK);
    await vi.advanceTimersByTimeAsync(1000);
    expect(session.getStatus().notWritten).toBe(FULL_DISK);
    await vi.advanceTimersByTimeAsync(1000);
    expect(session.getStatus().notWritten).toBe(null);
    expect(checked).toHaveBeenCalledTimes(1);
    expect(draftApi.editorUpdate).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toEqual(["update draft-1 A"]);
  });

  it("lets a later change take the place of one waiting to be tried again", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-1", quiz("A"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    session.write("draft-1", quiz("B"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toEqual(["update draft-1 B"]);
    expect(session.getStatus().notWritten).toBe(null);
  });

  it("lets go of a write for a draft another has replaced, and of the old draft's failing changes", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-1", quiz("A"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    expect(session.getStatus().notWritten).toBe(FULL_DISK);
    // Opening another quiz cancelled keeps the change trying.
    await session.exclusive("open", async () => false);
    expect(session.getStatus().notWritten).toBe(FULL_DISK);
    await session.exclusive("open", async (api) => {
      const opened = await api.editorOpen("other", PICKER);
      session.replaced();
      return opened;
    });
    expect(session.getStatus().notWritten).toBe(null);
    // A write of the old quiz given late is refused and dropped, not tried again.
    session.write("draft-1", quiz("A2"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toEqual(["update draft-1 A2"]);
    expect(session.getStatus().notWritten).toBe(null);
  });

  it("loads the draft with the change that could not be written yet", async () => {
    const { draftApi } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    draftApi.editorUpdate.mockRejectedValue(FULL_DISK);
    session.write("draft-1", quiz("Edited"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    const loaded = await session.load();
    expect(loaded?.quiz.title).toBe("Edited");
    expect(loaded?.unsaved).toBe(true);
    draftApi.editorUpdate.mockResolvedValue([]);
    await vi.advanceTimersByTimeAsync(1000);
    expect((await session.load())?.quiz.title).toBe("draft-1");
  });

  it("tells its listeners when it gets busy and when writing fails", async () => {
    const { draftApi } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    const seen: unknown[] = [];
    const stop = session.subscribe(() => seen.push({ ...session.getStatus() }));
    await session.exclusive("testRun", async () => undefined);
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-1", quiz("A"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(1000);
    stop();
    expect(seen).toEqual([
      { busy: "testRun", notWritten: null },
      { busy: null, notWritten: null },
      { busy: null, notWritten: FULL_DISK },
      { busy: null, notWritten: null },
    ]);
  });
});
