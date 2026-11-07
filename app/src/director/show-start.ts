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

/**
 * What to ask before a new show starts, in order: that it ends the way back
 * to the show that can be resumed (`last`), and that `file`, the quiz file
 * it plays when known, has newer changes in the editor's draft.
 */
export function startQuestions(t: Translate, last: SessionSummary | null, draft: DraftState | null, file?: string): string[] {
  const questions: string[] = [];
  if (last) questions.push(t("replaceLastShowConfirm", { title: last.meta.quizTitle }));
  const stale = staleFile(draft);
  if (stale && file !== undefined && stale.sourcePath === file) questions.push(t("startStaleConfirm", { title: stale.title }));
  return questions;
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
   * Starts a new show with `call`, after asking what `startQuestions` asks
   * (`file`: the quiz file it plays, when known before it starts); one
   * declined question starts nothing.
   */
  start: (call: () => Promise<unknown>, file?: string) => void;
  /** Resumes the last show with `call`, which asks nothing. */
  resume: (call: () => Promise<unknown>) => void;
}

/**
 * One show start at a time, so a double click opens a quiz once, also while
 * a question about it is open. The backend also refuses a second quiz file
 * start meanwhile (`showStarting`), which needs no word here.
 */
export function useShowStart(): ShowStart {
  const { t } = useApp();
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
      const questions = startQuestions(t, latest.current.last.session, latest.current.draft, file);
      const asked = questions.reduce<Promise<boolean>>((yes, question) => yes.then((go) => go && confirmAction(t, question)), Promise.resolve(true));
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
  const resume = useCallback(
    (call: () => Promise<unknown>) => {
      if (busy.current) return;
      busy.current = true;
      begin(call);
    },
    [begin],
  );
  return { starting, last, draft, start, resume };
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
