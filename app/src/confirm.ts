import { ask } from "@tauri-apps/plugin-dialog";

/**
 * Asks the quizmaster to confirm `message` in the system's own Yes/No
 * dialog, which no window's state can hide; true for Yes. The windows open
 * no file dialog themselves: the backend picks files (A21).
 */
export function confirmAction(message: string): Promise<boolean> {
  return ask(message, { kind: "warning" });
}
