import { ask, message } from "@tauri-apps/plugin-dialog";
import type { Translate } from "./i18n";

/**
 * Asks the quizmaster to confirm `message` in the system's own dialog,
 * which no window's state can hide, with buttons in the UI's language (the
 * system words them in English on macOS); true for Yes. The windows open no
 * file dialog themselves: the backend picks files.
 */
export function confirmAction(t: Translate, message: string): Promise<boolean> {
  return ask(message, { kind: "warning", okLabel: t("yes"), cancelLabel: t("no") });
}

/** What the quizmaster chose to do with unsaved changes. */
export type SaveChoice = "save" | "discard" | "cancel";

/**
 * Asks, in the system's own dialog, whether to save the unsaved changes
 * `question` names before they would be lost: Save, Don't save or Cancel.
 * Closing the dialog cancels.
 */
export async function confirmSave(t: Translate, question: string): Promise<SaveChoice> {
  const labels = { yes: t("saveQuiz"), no: t("dontSave"), cancel: t("cancel") };
  // With its own labels the dialog answers with the label pressed.
  const pressed = await message(question, { kind: "warning", buttons: labels });
  return saveChoice(pressed, labels);
}

/** The choice for the dialog's answer `pressed`, given its button `labels`. */
export function saveChoice(pressed: string, labels: { yes: string; no: string }): SaveChoice {
  return pressed === labels.yes ? "save" : pressed === labels.no ? "discard" : "cancel";
}
