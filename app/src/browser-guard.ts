// Keeps the windows from acting as a web browser during a show (A11). A
// stray F5 or Ctrl+R would reload the Director (cues and the sound board
// stop), Alt+Left would leave the page, Ctrl+P or Ctrl+F would open browser
// dialogs, a right click a browser menu, and a file dropped where nothing
// takes it would replace the window with that file. The backend keeps the
// windows on the app's pages as well (`app_windows::navigation_guard`).

import { isTypingTarget, keyCombo } from "./keyboard";

/** The webview's own keys, as `keyCombo` writes them. The app's handlers still get them (the editor's F5, Ctrl+F, Ctrl+S). */
const BROWSER_KEYS: ReadonlySet<string> = new Set([
  "F5",
  "Shift+F5",
  "Ctrl+F5",
  "Ctrl+R",
  "Ctrl+Shift+R",
  "BrowserRefresh",
  "Alt+ArrowLeft",
  "Alt+ArrowRight",
  "BrowserBack",
  "BrowserForward",
  "Ctrl+F",
  "Ctrl+G",
  "Ctrl+Shift+G",
  "F3",
  "Shift+F3",
  "Ctrl+P",
  "Ctrl+Shift+P",
  "Ctrl+S",
  "Ctrl+U",
  "Ctrl+J",
  "Ctrl+H",
  "F7",
]);

/** Keys that move the caret by words in a text field on macOS (Option+arrows), and go back or forward elsewhere. */
const WORD_KEYS: ReadonlySet<string> = new Set(["Alt+ArrowLeft", "Alt+ArrowRight"]);

/** True for a key press the webview would act on as a browser. */
export function isBrowserKey(event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey" | "target">): boolean {
  const combo = keyCombo(event);
  return BROWSER_KEYS.has(combo) && !(WORD_KEYS.has(combo) && isTypingTarget(event.target));
}

/**
 * Turns the browser behaviour off in `target`, a window. The guard listens
 * as the event comes down to its target (capture), so a part that stops the
 * event on its way back up, as a popup menu does, cannot skip it (G4).
 */
export function installBrowserGuard(target: Pick<Window, "addEventListener">) {
  const capture = { capture: true };
  target.addEventListener(
    "keydown",
    (event) => {
      if (isBrowserKey(event)) event.preventDefault();
    },
    capture,
  );
  // Text fields keep their menu (cut, copy, paste); everywhere else it is the browser's.
  target.addEventListener(
    "contextmenu",
    (event) => {
      if (!isTypingTarget(event.target)) event.preventDefault();
    },
    capture,
  );
  // The drop zones take files themselves; a drop anywhere else does nothing.
  target.addEventListener("dragover", (event) => event.preventDefault(), capture);
  target.addEventListener("drop", (event) => event.preventDefault(), capture);
}
