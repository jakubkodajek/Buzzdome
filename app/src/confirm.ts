import { ask, message } from "@tauri-apps/plugin-dialog";
import type { Translate } from "./i18n";
import { createStore } from "./store";

/**
 * How many of the system's dialogs are open now. They hold the window's
 * keys but not the backend's events, so what comes in meanwhile (the
 * quizmaster's remote) waits for them rather than change what they ask
 * about.
 */
export const openDialogs = createStore(0);

/** `dialog`, counted in `openDialogs` while it is open. */
function counted<T>(dialog: () => Promise<T>): Promise<T> {
  openDialogs.set(openDialogs.get() + 1);
  return dialog().finally(() => openDialogs.set(openDialogs.get() - 1));
}

/**
 * Asks the quizmaster to confirm `message` in the system's own dialog,
 * which no window's state can hide, with buttons in the UI's language (the
 * system words them in English on macOS); true for Yes. A step that cannot
 * be undone names itself on the button (`ok`, e.g. "End show") beside
 * Cancel, rather than Yes and No. The windows open no file dialog
 * themselves: the backend picks files.
 */
export function confirmAction(t: Translate, message: string, ok?: string): Promise<boolean> {
  return counted(() => ask(message, { kind: "warning", okLabel: ok ?? t("yes"), cancelLabel: ok ? t("cancel") : t("no") }));
}

/** Tells the quizmaster, in the system's own dialog, why a step they asked for failed, where no window shows it (closing the app). */
export async function tellFailure(text: string): Promise<void> {
  await message(text, { kind: "error" });
}

/** Which of a three-button dialog's buttons was pressed (`askThreeWay`). */
type Pressed = "yes" | "no" | "cancel";

/**
 * Asks `question` in the system's own dialog with the buttons `labels`
 * (`yes`, `no` and `cancel`). Closing the dialog cancels.
 */
async function askThreeWay(question: string, labels: { yes: string; no: string; cancel: string }): Promise<Pressed> {
  // With its own labels the dialog answers with the label pressed.
  const pressed = await counted(() => message(question, { kind: "warning", buttons: labels }));
  return pressedButton(pressed, labels);
}

/** The button for the dialog's answer `pressed`, given its button `labels`. */
function pressedButton(pressed: string, labels: { yes: string; no: string }): Pressed {
  return pressed === labels.yes ? "yes" : pressed === labels.no ? "no" : "cancel";
}

/** What the quizmaster chose to do with unsaved changes. */
export type SaveChoice = "save" | "discard" | "cancel";

const SAVE_CHOICES: Record<Pressed, SaveChoice> = { yes: "save", no: "discard", cancel: "cancel" };

/**
 * Asks, in the system's own dialog, whether to save the unsaved changes
 * `question` names before they would be lost: Save, Don't save or Cancel.
 * Closing the dialog cancels.
 */
export async function confirmSave(t: Translate, question: string): Promise<SaveChoice> {
  return SAVE_CHOICES[await askThreeWay(question, { yes: t("saveQuiz"), no: t("dontSave"), cancel: t("cancel") })];
}

/** The choice for the dialog's answer `pressed`, given its button `labels`. */
export function saveChoice(pressed: string, labels: { yes: string; no: string }): SaveChoice {
  return SAVE_CHOICES[pressedButton(pressed, labels)];
}

/** Whether a resumed show goes on with its quiz as edited since it began. */
export type EditsChoice = "edits" | "asBegun" | "cancel";

const EDITS_CHOICES: Record<Pressed, EditsChoice> = { yes: "edits", no: "asBegun", cancel: "cancel" };

/**
 * Asks, in the system's own dialog, whether the resumed show goes on with
 * its quiz as edited since it began (`question`): With the edits, Without
 * them or Cancel. Closing the dialog cancels.
 */
export async function confirmEdits(t: Translate, question: string): Promise<EditsChoice> {
  return EDITS_CHOICES[await askThreeWay(question, { yes: t("resumeWithEdits"), no: t("resumeAsBegun"), cancel: t("cancel") })];
}
