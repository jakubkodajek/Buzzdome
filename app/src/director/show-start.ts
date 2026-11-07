import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { confirmAction } from "../confirm";
import { useLastSession, useLatest, type LastSession } from "../hooks";
import type { Translate } from "../i18n";
import { isAppError } from "../messages";
import type { EditorDraft, SessionSummary } from "../types";

/** What the Director knows of the quiz editor's draft: whether it holds changes its file does not have. */
export type DraftState = Pick<EditorDraft, "sourcePath" | "unsaved"> & { title: string };

/** The draft as the Director needs it, from what `editorLoad` gives. */
export function draftState(draft: EditorDraft | null): DraftState | null {
  return draft && { title: draft.quiz.title, sourcePath: draft.sourcePath, unsaved: draft.unsaved };
}

/** The draft's unsaved changes of a quiz file, which a show from that file would not have; null when it has none. */
export function staleFile(draft: DraftState | null): DraftState | null {
  return draft?.unsaved && draft.sourcePath !== null ? draft : null;
}

/** The draft when it holds unsaved changes of the quiz file `file`, which a show of that file would not have; else null. */
export function staleFor(draft: DraftState | null, file: string): DraftState | null {
  const stale = staleFile(draft);
  return stale?.sourcePath === file ? stale : null;
}

/** A question before a show starts, and the step it names on its button. */
export interface StartQuestion {
  text: string;
  ok: string;
}

/**
 * What to ask before a new show starts, in one question: that it ends the
 * way back to the show that can be resumed (`last`) unless that show was
 * finished (it loses nothing worth asking about), and that `file`, the
 * quiz file it plays when known, has newer changes in the editor's draft.
 * Null when neither applies.
 */
export function startQuestion(t: Translate, last: SessionSummary | null, draft: DraftState | null, file?: string): StartQuestion | null {
  const stale = file === undefined ? null : staleFor(draft, file);
  const replaces = last && !last.finished && t("replaceLastShowConfirm", { title: last.meta.quizTitle });
  const older = stale && t("startStaleConfirm", { title: stale.title });
  if (replaces && older) return { text: `${replaces}\n\n${older}`, ok: t("startNewShow") };
  if (replaces) return { text: replaces, ok: t("startNewShow") };
  return older ? { text: older, ok: t("startSavedVersion") } : null;
}

/** Starting a show from the Director. */
export interface ShowStart {
  /** A start is under way: every way to start one waits for it. */
  starting: boolean;
  /** The last show, which Resume offers while there is one, or why it cannot be read. */
  last: LastSession;
  /** The editor's draft as it was when the Director opened; null when there is none or it could not be read. */
  draft: DraftState | null;
  /**
   * Starts a new show with `call`, after asking what `startQuestion` asks
   * (`file`: the quiz file it plays, when known before it starts); a
   * declined question starts nothing.
   */
  start: (call: () => Promise<unknown>, file?: string) => void;
  /**
   * Starts a new show with `call`, which picks its quiz file and is false
   * when the quizmaster cancels, after asking what `startQuestion` asks of
   * a file not known yet. Once it started the file whose newer changes are
   * in the editor's draft, it says so: only then is the file known.
   */
  startPicked: (call: () => Promise<boolean>) => void;
  /** Resumes the last show with `call`, which asks nothing. */
  resume: (call: () => Promise<unknown>) => void;
}

/**
 * One show start at a time, so a double click opens a quiz once, also while
 * a question about it is open. The backend also refuses a second quiz file
 * start meanwhile (`showStarting`), which needs no word here.
 */
export function useShowStart(): ShowStart {
  const { t, caution } = useApp();
  const run = useRun();
  const last = useLastSession();
  const draft = useDraftState();
  const [starting, setStarting] = useState(false);
  // Also a ref: two clicks before the next render would both see `starting` false.
  const busy = useRef(false);
  const latest = useLatest({ last, draft });

  const begin = useCallback(
    (call: () => Promise<unknown>) => {
      setStarting(true);
      const settled = () => {
        busy.current = false;
        setStarting(false);
      };
      run(call().catch(quietWhileStarting).finally(settled));
    },
    [run],
  );
  const start = useCallback(
    (call: () => Promise<unknown>, file?: string) => {
      if (busy.current) return;
      busy.current = true;
      const question = startQuestion(t, latest.current.last.session, latest.current.draft, file);
      const asked = question ? confirmAction(t, question.text, question.ok) : Promise.resolve(true);
      const declined = () => {
        busy.current = false;
      };
      run(
        asked.then(
          (go) => (go ? begin(call) : declined()),
          (error: unknown) => {
            declined();
            throw error;
          },
        ),
      );
    },
    [t, run, begin, latest],
  );
  // A show opened from a file goes first in the recent files.
  const startPicked = useCallback(
    (call: () => Promise<boolean>) =>
      start(() =>
        call().then(async (started) => {
          const draft = latest.current.draft;
          if (!started || !staleFile(draft)) return;
          const [opened] = await api.recentFiles();
          const stale = opened === undefined ? null : staleFor(draft, opened);
          if (stale) caution({ text: t("startedStale", { title: stale.title }), detail: opened });
        }),
      ),
    [start, latest, caution, t],
  );
  const resume = useCallback(
    (call: () => Promise<unknown>) => {
      if (busy.current) return;
      busy.current = true;
      begin(call);
    },
    [begin],
  );
  return { starting, last, draft, start, startPicked, resume };
}

/**
 * The editor's draft, read once when the Director's show controls open
 * (they open again on the way back from the editor, the only place it
 * changes). One that cannot be read is no draft here: the editor says why
 * when it opens.
 */
function useDraftState(): DraftState | null {
  const [draft, setDraft] = useState<DraftState | null>(null);
  useEffect(() => {
    let alive = true;
    api.editorLoad().then(
      (loaded) => alive && setDraft(draftState(loaded)),
      () => alive && setDraft(null),
    );
    return () => {
      alive = false;
    };
  }, []);
  return draft;
}

/** Throws `error` on, unless the backend refused a start because another is under way. */
function quietWhileStarting(error: unknown) {
  if (!isAppError(error) || error.code !== "showStarting") throw error;
}
