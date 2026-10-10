// The editor's side of the backend's draft (`editor.rs`): every call on it
// in order (`DraftQueue`), one file operation at a time, and every change
// written, tried again while writing fails, until it is.

import type { api } from "../api";
import { isAppError } from "../messages";
import type { EditorDraft, Quiz, ValidationError } from "../types";
import { DraftQueue } from "./draft-queue";

/** The backend's draft commands; a test stands in for them. */
export type DraftApi = Pick<
  typeof api,
  | "editorLoad"
  | "editorNew"
  | "editorOpen"
  | "editorClose"
  | "editorUpdate"
  | "editorSave"
  | "editorAddMedia"
  | "editorImportSheet"
  | "editorExportSheet"
  | "editorTestRun"
>;

/** What the editor does with its file, one at a time (A16). */
export type DraftOperation = "new" | "open" | "close" | "save" | "media" | "import" | "export" | "testRun";

export interface DraftStatus {
  /** The file operation under way; no other starts meanwhile. */
  busy: DraftOperation | null;
  /** Why the latest change is not in the draft (an `AppError`), while writing it fails; null once it is written. */
  notWritten: unknown;
}

/** What `exclusive` gives when another file operation is under way: the operation did not run. */
export const BUSY = Symbol("busy");

/** How long a change that could not be written waits before it is tried again. */
export const RETRY_AFTER_MS = 2000;

interface Write {
  draft: string;
  quiz: Quiz;
  unsaved: boolean;
  checked: (problems: ValidationError[]) => void;
}

/**
 * The backend refused a write for a draft that is not its current one:
 * another replaced it, or none is open (`AppError::DraftChanged`, `NoDraft`).
 */
function isRefused(error: unknown): boolean {
  return isAppError(error) && (error.code === "draftChanged" || error.code === "noDraft");
}

/** A value kept for the draft it was set in (`useDraftState`, A18). */
export interface DraftHeld<T> {
  draftId: string | null;
  value: T | null;
}

/** What `held` gives in the draft `draftId`: nothing once another draft is open. */
export function heldIn<T>(held: DraftHeld<T>, draftId: string | null): T | null {
  return held.draftId === draftId ? held.value : null;
}

export class DraftSession {
  private readonly api: DraftApi;
  private readonly retryAfterMs: number;
  private readonly queue: DraftQueue<Write>;
  private status: DraftStatus = { busy: null, notWritten: null };
  private readonly listeners = new Set<() => void>();
  /** The last change given; the draft has it once a write of it (or of a later one) works. */
  private latest: Write | null = null;
  /** The draft the editor has open: the one it loaded, or the one that replaced it. */
  private open: string | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;

  constructor(draftApi: DraftApi, retryAfterMs = RETRY_AFTER_MS) {
    this.api = draftApi;
    this.retryAfterMs = retryAfterMs;
    this.queue = new DraftQueue((write) => this.store(write));
  }

  /** For `useSyncExternalStore`. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getStatus = (): DraftStatus => this.status;

  /**
   * The draft, after every write given before. A change that could not be
   * written yet is the editor's latest, so the draft comes with it (the
   * editor left meanwhile, say).
   */
  async load(): Promise<EditorDraft | null> {
    const draft = await this.queue.run(() => this.api.editorLoad());
    this.open = draft?.id ?? null;
    const pending = this.latest;
    if (draft && pending?.draft === draft.id && this.status.notWritten !== null) {
      return { ...draft, quiz: pending.quiz, unsaved: pending.unsaved };
    }
    return draft;
  }

  /**
   * Runs `task` as the one file operation, after every write given before
   * it; gives `BUSY` without running it while another is under way.
   */
  async exclusive<T>(operation: DraftOperation, task: (draftApi: DraftApi) => Promise<T>): Promise<T | typeof BUSY> {
    if (this.status.busy) return BUSY;
    this.update({ busy: operation });
    try {
      return await this.queue.run(() => task(this.api));
    } finally {
      this.update({ busy: null });
    }
  }

  /** Whether an edit given now is kept: not while another draft takes the open one's place. */
  editable(): boolean {
    const busy = this.status.busy;
    return busy !== "new" && busy !== "open" && busy !== "close";
  }

  /**
   * The draft `draftId` (or none) took the open one's place: the old one's
   * changes that could not be written are let go, as the quizmaster chose.
   */
  replaced(draftId: string | null): void {
    this.cancelRetry();
    this.latest = null;
    this.open = draftId;
    this.update({ notWritten: null });
  }

  /**
   * Writes `quiz` into the draft `draft` after everything given before;
   * writes still waiting collapse into the latest. `checked` gets what keeps
   * the quiz from being played. A write that fails is tried again until it
   * or a later one works; one for a draft that another has replaced is let
   * go, but not one for the open draft: its change is not in the draft (G27).
   */
  write(draft: string, quiz: Quiz, unsaved: boolean, checked: (problems: ValidationError[]) => void): void {
    this.cancelRetry();
    const write = { draft, quiz, unsaved, checked };
    this.latest = write;
    this.queue.push(write);
  }

  private async store(write: Write): Promise<void> {
    try {
      write.checked(await this.api.editorUpdate(write.draft, write.quiz, write.unsaved));
      if (write === this.latest) this.update({ notWritten: null });
    } catch (error) {
      // A later write follows anyway, and a replaced draft's quiz is gone.
      if (write !== this.latest || (isRefused(error) && write.draft !== this.open)) return;
      this.update({ notWritten: error });
      this.retry = setTimeout(() => {
        this.retry = null;
        if (this.latest === write) this.queue.push(write);
      }, this.retryAfterMs);
    }
  }

  private cancelRetry() {
    if (this.retry !== null) clearTimeout(this.retry);
    this.retry = null;
  }

  private update(change: Partial<DraftStatus>) {
    const next = { ...this.status, ...change };
    if (next.busy === this.status.busy && next.notWritten === this.status.notWritten) return;
    this.status = next;
    for (const listener of this.listeners) listener();
  }
}
