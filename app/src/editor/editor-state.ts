// Everything the editor knows about the open quiz. Pure, like `history.ts`.

import { WARNING_PROBLEMS } from "../bindings/Constants";
import type { EditorDraft, Quiz, ValidationError } from "../types";
import { historyReducer, startHistory, type History, type HistoryAction } from "./history";

export interface OpenQuiz {
  /** The backend's name for the draft (`EditorDraft.id`), which every command on it gives. */
  draftId: string;
  history: History;
  /** The quiz as it is in the file at `sourcePath` (or as created); null when not known. */
  saved: Quiz | null;
  /** The file it was opened from or last saved to. */
  sourcePath: string | null;
  /** Save writes over `sourcePath` (`EditorDraft.savesInPlace`); else it asks where. */
  savesInPlace: boolean;
  /** Folder of the draft's media (see `MediaRootContext`). */
  mediaRoot: string;
  /** What keeps the quiz from being played, as the backend last checked. */
  problems: ValidationError[];
  /** What the file has that the quiz cannot keep, so saving leaves it out (`ReadQuiz::dropped`). */
  dropped: ValidationError[];
}

export type EditorState =
  | { status: "loading" }
  | { status: "empty" }
  /** The draft left from last time cannot be read (`error`); it stays until the quizmaster discards it. */
  | { status: "unreadable"; error: unknown }
  | ({ status: "open" } & OpenQuiz);

export type EditorAction =
  /**
   * A draft came from the backend (null: there is none). With `kept`, the
   * editor's state as it left that draft, which it goes on from (undo too)
   * while the draft still holds the same quiz.
   */
  | { type: "loaded"; draft: EditorDraft | null; kept?: OpenQuiz | null }
  /** The draft is being read again. */
  | { type: "loading" }
  /** The draft could not be read. */
  | { type: "unreadable"; error: unknown }
  | { type: "saved"; quiz: Quiz; path: string }
  /** The backend stored `quiz` and checked it. */
  | { type: "checked"; quiz: Quiz; problems: ValidationError[] }
  | HistoryAction;

export const LOADING: EditorState = { status: "loading" };

export function editorReducer(state: EditorState, action: EditorAction): EditorState {
  if (action.type === "loading") return LOADING;
  if (action.type === "unreadable") return { status: "unreadable", error: action.error };
  if (action.type === "loaded") {
    const { draft, kept } = action;
    if (!draft) return { status: "empty" };
    if (kept && kept.draftId === draft.id && sameData(kept.history.present, draft.quiz)) {
      const { history } = kept;
      // As the backend has it now; what the file holds as the editor knew it.
      const saved = draft.unsaved ? (kept.saved === history.present ? null : kept.saved) : history.present;
      return { ...kept, status: "open", saved, sourcePath: draft.sourcePath, savesInPlace: draft.savesInPlace, mediaRoot: draft.mediaRoot, problems: draft.problems, dropped: draft.dropped };
    }
    return {
      status: "open",
      draftId: draft.id,
      history: startHistory(draft.quiz),
      saved: draft.unsaved ? null : draft.quiz,
      sourcePath: draft.sourcePath,
      savesInPlace: draft.savesInPlace,
      mediaRoot: draft.mediaRoot,
      problems: draft.problems,
      dropped: draft.dropped,
    };
  }
  if (state.status !== "open") return state;
  switch (action.type) {
    case "saved":
      // The file no longer has what opening left out. Typing on after it starts a new undo step, so one step is the quiz as saved.
      return { ...state, history: { ...state.history, open: null }, saved: action.quiz, sourcePath: action.path, savesInPlace: true, dropped: [] };
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

/**
 * Whether two values hold the same data as JSON carries it: keys in any
 * order, a missing or undefined value the same as null.
 */
export function sameData(a: unknown, b: unknown): boolean {
  if (a === b || (a ?? null) === (b ?? null)) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((value, i) => sameData(value, b[i]));
  }
  const [x, y] = [a as Record<string, unknown>, b as Record<string, unknown>];
  return [...new Set([...Object.keys(x), ...Object.keys(y)])].every((key) => sameData(x[key], y[key]));
}

/** A quiz's problems apart: what to fix before it is played, and warnings of what may be a mistake but plays. */
export interface SplitProblems {
  problems: ValidationError[];
  warnings: ValidationError[];
}

const WARNINGS: ReadonlySet<string> = new Set(WARNING_PROBLEMS);

/** Splits `all` (`OpenQuiz.problems`) into problems and warnings (`WARNING_PROBLEMS`), each in its order. */
export function splitProblems(all: ValidationError[]): SplitProblems {
  const split: SplitProblems = { problems: [], warnings: [] };
  for (const problem of all) (WARNINGS.has(problem.problem.code) ? split.warnings : split.problems).push(problem);
  return split;
}

/** File › Info has something to tell: problems to fix (not warnings only), or what opening the file left out. */
export function needsInfo(quiz: Pick<OpenQuiz, "problems" | "dropped">): boolean {
  return splitProblems(quiz.problems).problems.length > 0 || quiz.dropped.length > 0;
}
