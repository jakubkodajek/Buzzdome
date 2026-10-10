import { ask, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useReducer, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useDispatch, useReport } from "../app-context";
import { useLatest } from "../hooks";
import type { StringKey } from "../i18n";
import { MEDIA_EXTENSIONS, type MediaKind } from "../media";
import type { Quiz, ValidationError } from "../types";
import { DraftQueue } from "./draft-queue";
import { editorReducer, isUnsaved, LOADING, type EditorState } from "./editor-state";
import type { Edit } from "./history";
import { readRecentFiles, rememberRecentFile } from "./recent";

const QUIZ_EXTENSION = "quiz";

const MEDIA_FILTER: Record<MediaKind, StringKey> = {
  image: "mediaPicture",
  audio: "mediaSound",
  video: "mediaVideo",
};

interface DraftWrite {
  quiz: Quiz;
  unsaved: boolean;
  checked: (problems: ValidationError[]) => void;
  failed: (error: unknown) => void;
}

// One draft for the whole app, so one queue, outliving the editor screen:
// whatever it wrote is on disk before the screen opens again and loads it.
const draftQueue = new DraftQueue<DraftWrite>(async (write) => {
  try {
    write.checked(await api.editorUpdate(write.quiz, write.unsaved));
  } catch (error) {
    write.failed(error);
  }
});

export interface QuizEditor {
  state: EditorState;
  /** Changes the quiz; `merge` joins quick changes of one field into one undo step. */
  edit: (edit: Edit, merge?: string) => void;
  undo: () => void;
  redo: () => void;
  // File operations resolve to whether they happened (not cancelled, not failed).
  newQuiz: () => Promise<boolean>;
  /**
   * Opens `path`, or a file the user picks. Resolves to what keeps the
   * opened quiz from being played (a file from an older version may break
   * newer rules), or false when nothing was opened.
   */
  openQuiz: (path?: string) => Promise<ValidationError[] | false>;
  /** Quiz files opened or saved lately, newest first. */
  recentFiles: string[];
  save: () => Promise<boolean>;
  saveAs: () => Promise<boolean>;
  closeQuiz: () => Promise<boolean>;
  /** Asks for a media file and copies it into the quiz; its reference, or null if none was picked. */
  pickMedia: (kinds: MediaKind[]) => Promise<string | null>;
  /** Plays the quiz as a show (see `onTestRun`), from its start or from slide `from`. */
  testRun: (from?: number) => Promise<boolean>;
}

/**
 * The quiz editor: the open quiz with undo, kept in the backend's draft
 * after every change, and the file operations. `onTestRun` is called once
 * a test show has started.
 */
export function useQuizEditor(onTestRun: () => void): QuizEditor {
  const { t } = useApp();
  const [state, dispatch] = useReducer(editorReducer, LOADING);
  const report = useLatest(useReport());
  const dispatchCommand = useDispatch();
  const [recentFiles, setRecentFiles] = useState(readRecentFiles);
  // The quiz the backend has; nothing is written back that came from it.
  const written = useRef<Quiz | null>(null);

  useEffect(() => {
    let alive = true;
    draftQueue.run(api.editorLoad).then((draft) => {
      if (!alive) return;
      written.current = draft?.quiz ?? null;
      dispatch({ type: "loaded", draft });
    }, report.current);
    return () => {
      alive = false;
    };
  }, [report]);

  const open = state.status === "open" ? state : null;
  const present = open?.history.present ?? null;
  const unsaved = open !== null && isUnsaved(open);
  useEffect(() => {
    if (!present || present === written.current) return;
    written.current = present;
    draftQueue.push({
      quiz: present,
      unsaved,
      checked: (problems) => dispatch({ type: "checked", quiz: present, problems }),
      failed: (error) => report.current(error),
    });
  }, [present, unsaved, report]);

  /** Runs a file operation; a failure is shown, not thrown, and gives false. */
  const attempt = <T>(operation: () => Promise<T | false>): Promise<T | false> =>
    operation().catch((error: unknown) => {
      report.current(error);
      return false as const;
    });

  const load = (draft: Awaited<ReturnType<typeof api.editorLoad>>) => {
    written.current = draft?.quiz ?? null;
    dispatch({ type: "loaded", draft });
  };

  /** Whether unsaved changes may be thrown away (asks if there are any). */
  const mayDiscard = async () =>
    !open || !unsaved || (await ask(t("discardChanges", { title: open.history.present.title || t("untitledQuiz") }), { kind: "warning" }));

  const saveTo = async (quiz: Quiz, path: string) => {
    await draftQueue.run(() => api.editorSave(quiz, path));
    dispatch({ type: "saved", quiz, path });
    setRecentFiles(rememberRecentFile(path));
    return true;
  };

  const saveAs = () =>
    attempt(async () => {
      if (!open) return false;
      const quiz = open.history.present;
      const path = await saveDialog({
        defaultPath: open.sourcePath ? withExtension(open.sourcePath) : `${fileStem(quiz.title || t("untitledQuiz"))}.${QUIZ_EXTENSION}`,
        filters: [{ name: t("quizFiles"), extensions: [QUIZ_EXTENSION] }],
      });
      return path ? saveTo(quiz, withExtension(path)) : false;
    });

  return {
    state,
    edit: (edit, merge) => dispatch({ type: "edit", edit, merge, at: Date.now() }),
    undo: () => dispatch({ type: "undo" }),
    redo: () => dispatch({ type: "redo" }),
    newQuiz: () =>
      attempt(async () => {
        if (!(await mayDiscard())) return false;
        load(await draftQueue.run(() => api.editorNew(t("untitledQuiz"))));
        return true;
      }),
    openQuiz: (given) =>
      attempt(async () => {
        if (!(await mayDiscard())) return false;
        const path = given ?? (await openDialog({ multiple: false, filters: [{ name: t("quizFiles"), extensions: [QUIZ_EXTENSION, "json"] }] }));
        if (typeof path !== "string") return false;
        const draft = await draftQueue.run(() => api.editorOpen(path));
        load(draft);
        setRecentFiles(rememberRecentFile(path));
        return draft.problems;
      }),
    recentFiles,
    save: () => {
      // A plain JSON quiz is saved as a new quiz file, never over the JSON.
      const path = open?.sourcePath;
      return open && path && hasExtension(path) ? attempt(() => saveTo(open.history.present, path)) : saveAs();
    },
    saveAs,
    closeQuiz: () =>
      attempt(async () => {
        if (!(await mayDiscard())) return false;
        await draftQueue.run(api.editorClose);
        load(null);
        return true;
      }),
    pickMedia: async (kinds) => {
      try {
        const extensions = kinds.flatMap((kind) => MEDIA_EXTENSIONS[kind]);
        const path = await openDialog({
          multiple: false,
          // Upper case too: some systems match file names case-sensitively.
          filters: [{ name: kinds.map((kind) => t(MEDIA_FILTER[kind])).join(", "), extensions: [...extensions, ...extensions.map((e) => e.toUpperCase())] }],
        });
        return typeof path === "string" ? await draftQueue.run(() => api.editorAddMedia(path)) : null;
      } catch (error) {
        report.current(error);
        return null;
      }
    },
    testRun: (from) =>
      attempt(async () => {
        if (!open) return false;
        const quiz = open.history.present;
        await draftQueue.run(() => api.editorTestRun(quiz));
        // The show runs now, whatever happens next: the Director shows it.
        onTestRun();
        if (from !== undefined) dispatchCommand({ type: "goTo", slide: from });
        return true;
      }),
  };
}

function hasExtension(path: string): boolean {
  return path.toLowerCase().endsWith(`.${QUIZ_EXTENSION}`);
}

/** `path` with the quiz file extension (a plain JSON quiz's name gets it in place of `.json`). */
function withExtension(path: string): string {
  return hasExtension(path) ? path : `${path.replace(/\.json$/i, "")}.${QUIZ_EXTENSION}`;
}

/** A quiz title as a file name. */
function fileStem(title: string): string {
  const stem = title.replace(/[\\/:*?"<>|\p{Cc}]/gu, "_").trim();
  return stem.replace(/^\.+|\.+$/g, "") || "quiz";
}
