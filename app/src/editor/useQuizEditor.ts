import { useCallback, useEffect, useReducer, useRef, useState, useSyncExternalStore } from "react";
import { api } from "../api";
import { useApp, useDispatch, useReport } from "../app-context";
import { useLatest } from "../hooks";
import { mediaPicker, type MediaKind } from "../media";
import { quizPicker } from "../quiz";
import type { EditorDraft, Quiz, SheetExport, SheetImport } from "../types";
import { BUSY, DraftSession, heldIn, type DraftApi, type DraftHeld, type DraftOperation, type DraftStatus } from "./draft-session";
import { editorReducer, isUnsaved, LOADING, type EditorState } from "./editor-state";
import type { Edit } from "./history";
import { readRecentFiles, rememberRecentFile } from "./recent";
import { listSeparator } from "./sheet";
import { confirmAction } from "../confirm";

// One draft for the whole app, so one session, outliving the editor screen:
// whatever it wrote is on disk before the screen opens again and loads it.
const drafts = new DraftSession(api);

export interface QuizEditor {
  state: EditorState;
  /** The file operation under way, and why the latest change is not in the draft, if it is not. */
  status: DraftStatus;
  /** Changes the quiz; `merge` joins quick changes of one field into one undo step. */
  edit: (edit: Edit, merge?: string) => void;
  undo: () => void;
  redo: () => void;
  // File operations resolve to whether they happened (not cancelled, not
  // failed, not refused while another one was under way).
  newQuiz: () => Promise<boolean>;
  /**
   * Opens `path`, or a file the user picks. Resolves to the draft opened (a
   * file from an older version may break newer rules, or have what this
   * version leaves out), or false when nothing was opened.
   */
  openQuiz: (path?: string) => Promise<EditorDraft | false>;
  /** Quiz files opened or saved lately, newest first. */
  recentFiles: string[];
  save: () => Promise<boolean>;
  saveAs: () => Promise<boolean>;
  closeQuiz: () => Promise<boolean>;
  /** Loads the draft again, after it could not be read. */
  reload: () => void;
  /** Throws away a draft that cannot be read, once the quizmaster confirms it. */
  discardUnreadable: () => Promise<boolean>;
  /** Asks for a media file and copies it into the quiz; its reference, or null if none was picked. */
  pickMedia: (kinds: MediaKind[]) => Promise<string | null>;
  /** Plays the quiz as a show (see `onTestRun`), from its start or from slide `from`. */
  testRun: (from?: number) => Promise<boolean>;
  /** Asks for a spreadsheet and reads its slides, copying their media into the quiz, for the editor to insert. */
  importSheet: () => Promise<SheetImport | false>;
  /** Asks where to and writes `quiz` as a CSV file named after `name`, with its media next to it. */
  exportSheet: (quiz: Quiz, name: string) => Promise<SheetExport | false>;
}

/**
 * The quiz editor: the open quiz with undo, kept in the backend's draft
 * after every change, and the file operations. `onTestRun` is called once
 * a test show has started.
 */
export function useQuizEditor(onTestRun: () => void): QuizEditor {
  const { t, lang, notify } = useApp();
  const [state, dispatch] = useReducer(editorReducer, LOADING);
  const status = useSyncExternalStore(drafts.subscribe, drafts.getStatus);
  const report = useLatest(useReport());
  const dispatchCommand = useDispatch();
  const [recentFiles, setRecentFiles] = useState(readRecentFiles);
  // The quiz the backend has; nothing is written back that came from it.
  const written = useRef<Quiz | null>(null);
  const [loads, setLoads] = useState(0);

  useEffect(() => {
    let alive = true;
    drafts.load().then(
      (draft) => {
        if (!alive) return;
        written.current = draft?.quiz ?? null;
        dispatch({ type: "loaded", draft });
      },
      // Not "no draft": the quizmaster chooses what becomes of it (A17).
      (error: unknown) => {
        if (alive) dispatch({ type: "unreadable", error });
      },
    );
    return () => {
      alive = false;
    };
  }, [loads]);

  const open = state.status === "open" ? state : null;
  const draftId = open?.draftId ?? null;
  const present = open?.history.present ?? null;
  const unsaved = open !== null && isUnsaved(open);
  useEffect(() => {
    if (!draftId || !present || present === written.current) return;
    written.current = present;
    drafts.write(draftId, present, unsaved, (problems) => dispatch({ type: "checked", quiz: present, problems }));
  }, [draftId, present, unsaved]);

  /**
   * Runs a file operation as the only one (A16); a failure is shown, not
   * thrown, and gives false, as does one refused while another runs: the
   * quizmaster is told why nothing happened (G28).
   */
  const attempt = <T>(operation: DraftOperation, task: (draftApi: DraftApi) => Promise<T | false>): Promise<T | false> =>
    drafts.exclusive(operation, task).then(
      (result) => {
        if (result !== BUSY) return result;
        notify(t("editorBusy"));
        return false as const;
      },
      (error: unknown) => {
        report.current(error);
        return false as const;
      },
    );

  /** Puts `draft` (or none) in place of the open one. */
  const load = (draft: EditorDraft | null) => {
    drafts.replaced(draft?.id ?? null);
    written.current = draft?.quiz ?? null;
    dispatch({ type: "loaded", draft });
  };

  /** Whether unsaved changes may be thrown away (asks if there are any). */
  const mayDiscard = async () =>
    !open || !unsaved || (await confirmAction(t, t("discardChanges", { title: open.history.present.title || t("untitledQuiz") })));

  /** Saves over the quiz file the draft came from, unless `saveAs`, else where the quizmaster picks. */
  const save = (saveAs: boolean) =>
    attempt("save", async (draftApi) => {
      if (!open) return false;
      const quiz = open.history.present;
      const picker = quizPicker(t, fileStem(quiz.title || t("untitledQuiz")));
      const path = await draftApi.editorSave(open.draftId, quiz, saveAs, picker);
      if (path === null) return false;
      dispatch({ type: "saved", quiz, path });
      setRecentFiles(rememberRecentFile(path));
      return true;
    });

  /** Discards the draft, also one that cannot be read, once `confirmed`. */
  const discard = (confirmed: () => Promise<boolean>) =>
    attempt("close", async (draftApi) => {
      if (!(await confirmed())) return false;
      await draftApi.editorClose();
      load(null);
      return true;
    });

  // Edits given while another draft takes the open one's place would be lost with it.
  const editable = () => drafts.editable();

  return {
    state,
    status,
    edit: (edit, merge) => {
      if (editable()) dispatch({ type: "edit", edit, merge, at: Date.now() });
    },
    undo: () => {
      if (editable()) dispatch({ type: "undo" });
    },
    redo: () => {
      if (editable()) dispatch({ type: "redo" });
    },
    newQuiz: () =>
      attempt("new", async (draftApi) => {
        if (!(await mayDiscard())) return false;
        load(await draftApi.editorNew(t("untitledQuiz")));
        return true;
      }),
    openQuiz: (given) =>
      attempt("open", async (draftApi) => {
        if (!(await mayDiscard())) return false;
        const draft = await draftApi.editorOpen(given ?? null, quizPicker(t));
        if (!draft) return false;
        load(draft);
        if (draft.sourcePath) setRecentFiles(rememberRecentFile(draft.sourcePath));
        return draft;
      }),
    recentFiles,
    save: () => save(false),
    saveAs: () => save(true),
    closeQuiz: () => discard(mayDiscard),
    reload: () => {
      dispatch({ type: "loading" });
      setLoads((n) => n + 1);
    },
    discardUnreadable: () => discard(() => confirmAction(t, t("discardUnreadableDraft"))),
    pickMedia: async (kinds) => {
      const picked = await attempt("media", async (draftApi) => {
        if (!open) return false;
        return (await draftApi.editorAddMedia(open.draftId, kinds, mediaPicker(t, kinds))) ?? false;
      });
      return picked === false ? null : picked;
    },
    importSheet: () =>
      attempt("import", async (draftApi) => {
        if (!open) return false;
        return (await draftApi.editorImportSheet(open.draftId, { filter: t("sheetFiles") })) ?? false;
      }),
    exportSheet: (quiz, name) =>
      attempt("export", async (draftApi) => {
        if (!open) return false;
        const picker = { filter: t("csvFile"), fileName: fileStem(name) };
        return (await draftApi.editorExportSheet(open.draftId, quiz, listSeparator(lang), picker)) ?? false;
      }),
    testRun: (from) =>
      attempt("testRun", async (draftApi) => {
        if (!open) return false;
        await draftApi.editorTestRun(open.draftId, open.history.present);
        // The show runs now, whatever happens next: the Director shows it.
        onTestRun();
        if (from !== undefined) dispatchCommand({ type: "goTo", slide: from });
        return true;
      }),
  };
}

/**
 * State that belongs to the open draft, such as a clipboard: once another
 * draft is open it reads as null, so nothing of one quiz (the name of one
 * of its media files) lands in another (A18).
 */
export function useDraftState<T>(draftId: string | null): [T | null, (value: T | null) => void] {
  const [held, setHeld] = useState<DraftHeld<T>>({ draftId, value: null });
  const set = useCallback((value: T | null) => setHeld({ draftId, value }), [draftId]);
  return [heldIn(held, draftId), set];
}

/** The id of the draft open in `editor`, if one is. */
export function openDraftId(editor: QuizEditor): string | null {
  return editor.state.status === "open" ? editor.state.draftId : null;
}

/** A quiz title as a file name, for the backend to add the extension to. */
function fileStem(title: string): string {
  const stem = title.replace(/[\\/:*?"<>|\p{Cc}]/gu, "_").trim();
  return stem.replace(/^\.+|\.+$/g, "") || "quiz";
}
