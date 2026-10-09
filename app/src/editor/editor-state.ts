// Everything the editor knows about the open quiz. Pure, like `history.ts`.

import type { EditorDraft, Quiz, ValidationError } from "../types";
import { historyReducer, startHistory, type History, type HistoryAction } from "./history";

export interface OpenQuiz {
  history: History;
  /** The quiz as it is in the file at `sourcePath` (or as created); null when not known. */
  saved: Quiz | null;
  /** The file it was opened from or last saved to. */
  sourcePath: string | null;
  /** Folder of the draft's media (see `MediaRootContext`). */
  mediaRoot: string;
  /** What keeps the quiz from being played, as the backend last checked. */
  problems: ValidationError[];
}

export type EditorState = { status: "loading" } | { status: "empty" } | ({ status: "open" } & OpenQuiz);

export type EditorAction =
  /** A draft came from the backend (null: there is none). */
  | { type: "loaded"; draft: EditorDraft | null }
  | { type: "saved"; quiz: Quiz; path: string }
  /** The backend stored `quiz` and checked it. */
  | { type: "checked"; quiz: Quiz; problems: ValidationError[] }
  | HistoryAction;

export const LOADING: EditorState = { status: "loading" };

export function editorReducer(state: EditorState, action: EditorAction): EditorState {
  if (action.type === "loaded") {
    const { draft } = action;
    if (!draft) return { status: "empty" };
    return {
      status: "open",
      history: startHistory(draft.quiz),
      saved: draft.unsaved ? null : draft.quiz,
      sourcePath: draft.sourcePath,
      mediaRoot: draft.mediaRoot,
      problems: draft.problems,
    };
  }
  if (state.status !== "open") return state;
  switch (action.type) {
    case "saved":
      return { ...state, saved: action.quiz, sourcePath: action.path };
    case "checked":
      // A check of a quiz that has changed since is already out of date.
      return action.quiz === state.history.present ? { ...state, problems: action.problems } : state;
    default: {
      const history = historyReducer(state.history, action);
      return history === state.history ? state : { ...state, history };
    }
  }
}

/**
 * The quiz has changes that are not in its file. A new quiz counts as saved
 * until it is changed: there is nothing to lose yet.
 */
export function isUnsaved(open: OpenQuiz): boolean {
  return open.history.present !== open.saved;
}
