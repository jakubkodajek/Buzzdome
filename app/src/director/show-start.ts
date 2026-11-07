import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { confirmAction, confirmEdits } from "../confirm";
import { loadDraftAfterWrites } from "../editor/useQuizEditor";
import { dayAndTime } from "../format";
import { useLastSession, useLatest, useRecentFiles, type LastSession, type RecentFiles } from "../hooks";
import type { Lang, Translate } from "../i18n";
import { errorText, isAppError } from "../messages";
import { quizPicker } from "../quiz";
import type { EditorDraft, SessionSummary, ShowPosition } from "../types";

/** What the Director knows of the quiz editor's draft: whether it holds changes its file does not have. */
export type DraftState = Pick<EditorDraft, "sourcePath" | "unsaved"> & { title: string };

/** The draft as the Director needs it, from what `loadDraftAfterWrites` gives. */
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
 * quiz file it plays if any, has newer changes in the editor's draft.
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

/** Where the show `position` stands, in words. */
export function positionText(t: Translate, position: ShowPosition): string {
  switch (position.at) {
    case "beforeSlides":
      return t("positionBeforeSlides");
    case "question":
      return t("positionQuestion", { n: position.number, total: position.count });
    case "slide":
      return t("positionSlide", { n: position.number, total: position.count });
    case "finalRanking":
      return t("positionFinalRanking");
  }
}

/** What Resume says of the last show `last`: its quiz, where it stands when known, and when it began. */
export function resumeInfo(t: Translate, lang: Lang, last: SessionSummary): string {
  const params = { title: last.meta.quizTitle, time: dayAndTime(last.meta.startedMs, lang) };
  return last.position ? t("resumeInfoAt", { ...params, position: positionText(t, last.position) }) : t("resumeInfo", params);
}

/** Starting a show from the Director. */
export interface ShowStart {
  /** A start is under way: every way to start one waits for it. */
  starting: boolean;
  /** The last show, which Resume offers while there is one, or why it cannot be read. */
  last: LastSession;
  /** The editor's draft as it was when the Director opened; null when there is none or it could not be read. */
  draft: DraftState | null;
  /** The quiz files a show starts from in a click (`startFile`), latest first. */
  recent: RecentFiles;
  /** Starts a new show with `call`, which plays no quiz file, after asking what `startQuestion` asks; a declined question starts nothing. */
  start: (call: () => Promise<unknown>) => void;
  /**
   * Starts a show of `file`, one of `recent`, or of a quiz file the
   * quizmaster picks when none is given, after asking what
   * `startQuestion` asks of that file; a cancelled pick or a declined
   * question starts nothing.
   */
  startFile: (file?: string) => void;
  /**
   * Resumes the last show with `call`, after asking whether it goes on with
   * its quiz as saved since it began, if it was (`SessionSummary.edited`):
   * `takeEdits` is the answer. Edits the show cannot take are said, and the
   * show resumes without them; a cancelled question resumes nothing.
   */
  resume: (call: (takeEdits: boolean) => Promise<unknown>) => void;
}

/**
 * One show start at a time, so a double click opens a quiz once, also while
 * its picker or a question about it is open. The backend also refuses a
 * second quiz file start meanwhile (`showStarting`), which needs no word here.
 */
export function useShowStart(): ShowStart {
  const { t, caution } = useApp();
  const run = useRun();
  const last = useLastSession();
  const draft = useDraftState();
  const recent = useRecentFiles();
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
  // The one way to a new show: the file it plays, picked first if need be
  // (null when the quizmaster cancels), then the question, then the start.
  const startOf = useCallback(
    <F extends string | undefined>(file: () => Promise<F | null>, call: (file: F) => Promise<unknown>) => {
      if (busy.current) return;
      busy.current = true;
      const declined = () => {
        busy.current = false;
      };
      const ask = async (found: F | null) => {
        if (found === null) return declined();
        const question = startQuestion(t, latest.current.last.session, latest.current.draft, found);
        const go = question === null || (await confirmAction(t, question.text, question.ok));
        return go ? begin(() => call(found)) : declined();
      };
      run(
        file()
          .then(ask)
          .catch((error: unknown) => {
            declined();
            throw error;
          }),
      );
    },
    [t, run, begin, latest],
  );
  const start = useCallback((call: () => Promise<unknown>) => startOf(() => Promise.resolve(undefined), call), [startOf]);
  // Read again either way: a file that is gone leaves the list, and one picked goes first in it.
  const { reload } = recent;
  const startFile = useCallback(
    (file?: string) =>
      startOf(
        () => (file === undefined ? api.pickQuizFile(quizPicker(t)).finally(reload) : Promise.resolve(file)),
        (path: string) => api.startShowFromRecent(path).finally(reload),
      ),
    [startOf, t, reload],
  );
  // The file is looked at again as Resume is pressed: it may have been
  // saved since the Director read the last show.
  const resume = useCallback(
    (call: (takeEdits: boolean) => Promise<unknown>) => {
      if (busy.current) return;
      busy.current = true;
      const ask = async (session: SessionSummary | null) => {
        const edited = session?.edited;
        if (edited?.refused) caution(errorText(t, { code: "editsNotTaken", change: edited.refused }));
        const choice = session && edited && !edited.refused ? await confirmEdits(t, t("resumeEditedConfirm", { title: session.meta.quizTitle })) : "asBegun";
        if (choice === "cancel") busy.current = false;
        else begin(() => call(choice === "edits"));
      };
      run(
        api
          .lastSession()
          .then(ask)
          .catch((error: unknown) => {
            busy.current = false;
            throw error;
          }),
      );
    },
    [t, caution, run, begin],
  );
  return { starting, last, draft, recent, start, startFile, resume };
}

/**
 * The editor's draft, read once when the Director's show controls open
 * (they open again on the way back from the editor, the only place it
 * changes), after the editor's changes still being written. One that cannot be read is no draft here: the editor says why
 * when it opens.
 */
function useDraftState(): DraftState | null {
  const [draft, setDraft] = useState<DraftState | null>(null);
  useEffect(() => {
    let alive = true;
    loadDraftAfterWrites().then(
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
