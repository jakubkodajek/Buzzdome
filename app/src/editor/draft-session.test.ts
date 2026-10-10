import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EditorDraft, Quiz } from "../types";
import { BUSY, DraftSession, heldIn, type DraftApi } from "./draft-session";

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
/** The recent files the backend lists, opened by their place. */
const RECENT = ["big", "other"];
const [BIG, OTHER] = [0, 1];

/** A stand-in for the backend: one current draft (or none), and a record of every call. */
function fakeApi(start = "draft-1") {
  let current: string | null = start;
  const calls: string[] = [];
  const draftApi = {
    editorLoad: vi.fn(async () => (current === null ? null : draft(current))),
    editorNew: vi.fn(async () => draft((current = "draft-new"))),
    editorOpen: vi.fn(async (recent: number | null) => draft((current = `draft-${recent === null ? "picked" : RECENT[recent]}`))),
    editorClose: vi.fn(async () => {
      current = null;
    }),
    editorUpdate: vi.fn(async (id: string, written: Quiz) => {
      calls.push(`update ${id} ${written.title}`);
      if (current === null) throw { code: "noDraft" };
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
  /** Makes `id` the current draft behind the editor's back. */
  const replaceBehind = (id: string) => (current = id);
  return { draftApi, calls, replaceBehind };
}

describe("DraftSession", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("starts no file operation while another runs, so Ctrl+S cannot save the old quiz into the one opening", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi);
    const opening = deferred<EditorDraft>();
    draftApi.editorOpen.mockImplementationOnce(() => opening.promise);
    const open = session.exclusive("open", (api) => api.editorOpen(BIG, PICKER));
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
      const opened = await api.editorOpen(OTHER, PICKER);
      session.replaced(opened?.id ?? null);
      return opened;
    });
    expect(session.getStatus().notWritten).toBe(null);
    // A write of the old quiz given late is refused and dropped, not tried again.
    session.write("draft-1", quiz("A2"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toEqual(["update draft-1 A2"]);
    expect(session.getStatus().notWritten).toBe(null);
  });

  it("lets go of a late write once the draft is closed, rather than trying it again for ever", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    await session.load();
    await session.exclusive("close", async (api) => {
      await api.editorClose();
      session.replaced(null);
    });
    session.write("draft-1", quiz("Late"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toEqual(["update draft-1 Late"]);
    expect(session.getStatus().notWritten).toBe(null);
  });

  it("counts a refusal for the open draft as a change not written, and keeps trying", async () => {
    const { draftApi, calls, replaceBehind } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    await session.load();
    replaceBehind("draft-2");
    session.write("draft-1", quiz("Shown"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    expect(session.getStatus().notWritten).toEqual({ code: "draftChanged" });
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toEqual(["update draft-1 Shown", "update draft-1 Shown"]);
  });

  it("tries the new draft's changes again once it replaced the old one, and loads it without the old one's", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    await session.load();
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-1", quiz("Old"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    await session.exclusive("open", async (api) => {
      const opened = await api.editorOpen(OTHER, PICKER);
      session.replaced(opened?.id ?? null);
    });
    expect((await session.load())?.quiz.title).toBe("draft-other");
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-other", quiz("New"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    expect(session.getStatus().notWritten).toBe(FULL_DISK);
    await vi.advanceTimersByTimeAsync(1000);
    expect(session.getStatus().notWritten).toBe(null);
    expect(calls).toEqual(["update draft-other New"]);
  });

  it("runs a retry that comes due during an open after it, and lets it go once the open replaced the draft", async () => {
    const { draftApi, calls } = fakeApi();
    const session = new DraftSession(draftApi, 1000);
    await session.load();
    draftApi.editorUpdate.mockRejectedValueOnce(FULL_DISK);
    session.write("draft-1", quiz("A"), true, () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    const opening = deferred<EditorDraft>();
    draftApi.editorOpen.mockImplementationOnce(() => opening.promise);
    const open = session.exclusive("open", async (api) => {
      const opened = await api.editorOpen(BIG, PICKER);
      session.replaced(opened?.id ?? null);
    });
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toEqual([]);
    opening.resolve(draft("draft-big"));
    await open;
    await vi.advanceTimersByTimeAsync(5000);
    // The retry was of the old draft, refused once it was replaced.
    expect(calls).toEqual(["update draft-1 A"]);
    expect(session.getStatus().notWritten).toBe(null);
  });

  it("keeps edits only while no other draft is taking the open one's place", async () => {
    const { draftApi } = fakeApi();
    const session = new DraftSession(draftApi);
    const seen: boolean[] = [];
    for (const operation of ["new", "open", "close", "save", "media", "import", "export", "testRun"] as const) {
      await session.exclusive(operation, async () => seen.push(session.editable()));
    }
    expect(seen).toEqual([false, false, false, true, true, true, true, true]);
    expect(session.editable()).toBe(true);
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

describe("heldIn", () => {
  it("gives what was kept in the open draft, and nothing once another is open (A18)", () => {
    const copied = { draftId: "draft-1", value: "media/a.png" };
    expect(heldIn(copied, "draft-1")).toBe("media/a.png");
    expect(heldIn(copied, "draft-2")).toBe(null);
    expect(heldIn(copied, null)).toBe(null);
  });
});
