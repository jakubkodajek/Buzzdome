// The question the Director asks when the app is asked to close while
// something would be left behind (`close-requested`, `CloseReason`).

import { api } from "./api";
import { confirmAction, confirmSave, tellFailure, type SaveChoice } from "./confirm";
import { loadDraftAfterWrites } from "./editor/useQuizEditor";
import type { Translate } from "./i18n";
import { errorText } from "./messages";
import { quizPicker } from "./quiz";
import type { CloseReason, EditorDraft, Picker, SavedDraft } from "./types";

/** What answering a close request uses: the backend and the system's dialogs. */
export interface CloseDeps {
  /** The editor's draft, after its changes still being written. */
  loadDraft: () => Promise<EditorDraft | null>;
  editorSave: (draft: string, quiz: EditorDraft["quiz"], saveAs: boolean, picker: Picker) => Promise<SavedDraft | null>;
  quitApp: () => Promise<void>;
  closeDeclined: () => Promise<void>;
  confirm: (question: string, ok: string) => Promise<boolean>;
  confirmSave: (question: string) => Promise<SaveChoice>;
  /** Shows why saving failed. */
  failed: (text: string) => Promise<unknown>;
}

/** The backend and the system's dialogs, worded by `t`. */
export function closeDeps(t: Translate): CloseDeps {
  return {
    loadDraft: loadDraftAfterWrites,
    editorSave: api.editorSave,
    quitApp: api.quitApp,
    closeDeclined: api.closeDeclined,
    confirm: (question, ok) => confirmAction(t, question, ok),
    confirmSave: (question) => confirmSave(t, question),
    failed: tellFailure,
  };
}

/**
 * Asks whether to close the app, naming everything `reason` says would be
 * left behind, then closes it or not. With an unsaved quiz and no show it
 * offers to save the quiz first (Save / Don't save / Cancel); during a show
 * the editor cannot save, so the question only says that the draft stays.
 */
export async function answerClose(t: Translate, reason: CloseReason, deps: CloseDeps): Promise<void> {
  const draft = reason.unsavedQuiz && !reason.show ? await deps.loadDraft() : null;
  let quit: boolean;
  if (draft) {
    const title = draft.quiz.title || t("untitledQuiz");
    const choice = await deps.confirmSave(t("quitSaveQuiz", { title }));
    quit = choice === "discard" || (choice === "save" && (await saved(t, draft, title, deps)));
  } else {
    const parts = [
      ...(reason.show ? [t("quitShowRuns")] : []),
      ...(reason.unsavedQuiz ? [t("quitQuizUnsaved")] : []),
      t("quitAnyway"),
    ];
    quit = await deps.confirm(parts.join(" "), t("closeApp"));
  }
  await (quit ? deps.quitApp() : deps.closeDeclined());
}

/** Saves the editor's draft as the editor's Save does; whether it was saved (not when the quizmaster cancels the file picker, or it failed, which is shown). */
async function saved(t: Translate, draft: EditorDraft, title: string, deps: CloseDeps): Promise<boolean> {
  try {
    return (await deps.editorSave(draft.id, draft.quiz, false, quizPicker(t, title))) !== null;
  } catch (error) {
    await deps.failed(errorText(t, error));
    return false;
  }
}
