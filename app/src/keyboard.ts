import { useEffect } from "react";
import { useDispatch } from "./app-context";
import type { Translate } from "./i18n";
import type { Command, GameView } from "./types";

/** The quizmaster's keys, as `KeyboardEvent.key` in lower case. Digits 1-0 are the keyboard buzzers. */
export const KEYS = {
  next: " ",
  right: "y",
  wrong: "n",
  clock: "p",
  leaderboard: "s",
  projector: "m",
} as const;

/** Keys that also go to the next step. */
const NEXT_KEYS: readonly string[] = [KEYS.next, "arrowright", "pagedown"];

/** How a key is written on the button it stands for. */
export function keyHint(t: Translate, key: string): string {
  return key === " " ? t("keySpace") : key.toUpperCase();
}

/** Input types that take typed text; the others (sliders, checkboxes, colours) take none. */
const TEXT_INPUT_TYPES: readonly string[] = ["text", "search", "email", "url", "tel", "password", "number"];

/** What a key press's target says about the keys it takes. */
type KeyTarget = Partial<Pick<HTMLInputElement, "tagName" | "type" | "isContentEditable">>;

/**
 * True when a key press is typed text: in a text field, a text area or
 * editable text. A slider, checkbox or list takes no typing, so the show's
 * keys still work after the quizmaster touched one (A5).
 */
export function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as KeyTarget | null;
  switch (element?.tagName) {
    case "INPUT":
      return TEXT_INPUT_TYPES.includes(element.type ?? "text");
    case "TEXTAREA":
      return true;
    default:
      return element?.isContentEditable === true;
  }
}

/** True when a key press goes to any form field, which keeps its keys (arrows in a list or on a slider). */
export function isFieldTarget(target: EventTarget | null): boolean {
  const tag = (target as KeyTarget | null)?.tagName;
  return tag === "INPUT" || tag === "SELECT" || isTypingTarget(target);
}

/** Whether the app runs on macOS, where Cmd does what Ctrl does elsewhere. */
const IS_MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);

/**
 * A key press written as the editor's `Action.keys` are, e.g. "Ctrl+S": modifiers in the order Ctrl, Alt,
 * Shift, then the key. Ctrl stands for Cmd on macOS (`mac`), where Control is written "Control": it moves
 * the caret in text fields there (Control+F, Control+H), so it stands for none of the app's keys.
 */
export function keyCombo(event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">, mac = IS_MAC): string {
  const parts: string[] = [];
  if (mac ? event.metaKey : event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (mac && event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(event.key.length === 1 ? event.key.toUpperCase() : event.key);
  return parts.join("+");
}

/** True for the keys a running show takes, whether or not the current step has a use for them. */
function isShowKey(key: string): boolean {
  const lower = key.toLowerCase();
  return /^[0-9]$/.test(key) || NEXT_KEYS.includes(lower) || [KEYS.right, KEYS.wrong, KEYS.clock, KEYS.leaderboard].some((k) => k === lower);
}

/** True for Enter held down on a button, which would press it again with every repeat (G13). */
export function isHeldButtonPress(event: Pick<KeyboardEvent, "key" | "repeat" | "target">): boolean {
  return event.repeat && event.key === "Enter" && (event.target as KeyTarget | null)?.tagName === "BUTTON";
}

/**
 * Quizmaster keys, the same in the Director and on the projector window.
 * `onToggleProjector` adds M, which opens or closes the projector from the
 * Director; the projector closes itself (`useProjectorCloseKeys`).
 */
export function useShortcuts(view: GameView | null, onToggleProjector?: () => void) {
  const dispatch = useDispatch();
  useEffect(() => {
    // Before anything else sees it, so no control on the way can let it through (G13).
    const onKeyFirst = (event: KeyboardEvent) => {
      if (isHeldButtonPress(event)) event.preventDefault();
    };
    const onKey = (event: KeyboardEvent) => {
      // A key a control already took (Space on a focused team chip) is not the show's.
      if (event.defaultPrevented || isTypingTarget(event.target) || event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key.toLowerCase() === KEYS.projector && onToggleProjector) {
        event.preventDefault();
        if (!event.repeat) onToggleProjector();
        return;
      }
      if (!view || !isShowKey(event.key)) return;
      // Taken even when the step has no use for it, so Space never presses a focused button mid-show.
      event.preventDefault();
      // A key held down sends one command, not one per repeat (A4).
      if (event.repeat) return;
      const command = commandForKey(event.key, view);
      if (command) dispatch(command);
    };
    // A slider keeps no focus once let go, so the arrow keys never move it unseen (A5).
    const onPointerUp = (event: PointerEvent) => {
      if (view && event.target instanceof HTMLInputElement && event.target.type === "range") event.target.blur();
    };
    window.addEventListener("keydown", onKeyFirst, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerup", onPointerUp);
    return () => {
      window.removeEventListener("keydown", onKeyFirst, true);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, [view, dispatch, onToggleProjector]);
}

/** True for a key press that closes the projector from itself: M or Esc, held or not, with no modifier. */
export function isProjectorCloseKey(event: Pick<KeyboardEvent, "key" | "repeat" | "metaKey" | "ctrlKey" | "altKey" | "defaultPrevented" | "target">): boolean {
  if (event.defaultPrevented || event.repeat || event.metaKey || event.ctrlKey || event.altKey || isTypingTarget(event.target)) return false;
  return event.key === "Escape" || event.key.toLowerCase() === KEYS.projector;
}

/**
 * M or Esc closes the projector from itself: full screen over the Director,
 * it is the window that has the keys (A13), also once its screen failed.
 */
export function useProjectorCloseKeys(close: () => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!isProjectorCloseKey(event)) return;
      event.preventDefault();
      close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);
}

/** The command a key stands for, or null when the current step does not take it (see `Controls`). */
export function commandForKey(key: string, view: GameView): Command | null {
  if (/^[0-9]$/.test(key)) {
    // Keyboard buzzers as in the original: 1..9 and 0 for buzzer 10.
    return { type: "press", buzzer: key === "0" ? 10 : Number(key), button: "big" };
  }
  const { controls } = view;
  const lower = key.toLowerCase();
  if (NEXT_KEYS.includes(lower)) return controls.next ? { type: "next" } : null;
  switch (lower) {
    case KEYS.right:
      return controls.judge ? { type: "judgeRight" } : null;
    case KEYS.wrong:
      return controls.judge ? { type: "judgeWrong" } : null;
    case KEYS.clock:
      if (controls.pauseClock) return { type: "pauseClock" };
      return controls.resumeClock ? { type: "resumeClock" } : null;
    case KEYS.leaderboard:
      return { type: "showLeaderboard", visible: !view.leaderboardVisible };
    default:
      return null;
  }
}
