import { ask } from "@tauri-apps/plugin-dialog";
import type { Translate } from "./i18n";

/**
 * Asks the quizmaster to confirm `message` in the system's own dialog,
 * which no window's state can hide, with buttons in the UI's language (the
 * system words them in English on macOS); true for Yes. The windows open no
 * file dialog themselves: the backend picks files (A21).
 */
export function confirmAction(t: Translate, message: string): Promise<boolean> {
  return ask(message, { kind: "warning", okLabel: t("yes"), cancelLabel: t("no") });
}
