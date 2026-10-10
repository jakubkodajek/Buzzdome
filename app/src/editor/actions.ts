// What the editor can do, in one place: the ribbon, the context menus and
// the keyboard all run the same actions.

import { useEffect, type ReactNode } from "react";
import { useLatest } from "../hooks";
import { isTypingTarget } from "../keyboard";

export interface Action {
  label: string;
  icon?: ReactNode;
  /** Key combinations as `keyCombo` writes them, e.g. "Ctrl+S"; the first is shown. */
  keys?: string[];
  /** The keys work while typing in a field too. */
  inFields?: boolean;
  /** Holding the keys down repeats the action. */
  repeats?: boolean;
  disabled?: boolean;
  /** A setting that is switched on. */
  checked?: boolean;
  run: () => void;
}

export type ActionId =
  | "file"
  | "newQuiz"
  | "openQuiz"
  | "save"
  | "saveAs"
  | "closeQuiz"
  | "exit"
  | "undo"
  | "redo"
  | "cut"
  | "copy"
  | "paste"
  | "delete"
  | "duplicate"
  | "moveUp"
  | "moveDown"
  | "selectAll"
  | "edit"
  | "newSlide"
  | "addQuestion"
  | "previous"
  | "next"
  | "first"
  | "last"
  | "insertPicture"
  | "insertVideo"
  | "insertSound"
  | "insertBackground"
  | "changeMedia"
  | "removeMedia"
  | "trimSound"
  | "trimVideo"
  | "normalizeSound"
  | "normalizeAll"
  | "toggleBackground"
  | "removeBackground"
  | "graphical"
  | "table"
  | "properties"
  | "findProperty"
  | "importSheet"
  | "sheetTemplate"
  | "exportSheet"
  | "notes"
  | "testRun"
  | "testRunHere";

export type Actions = Record<ActionId, Action>;

/**
 * A key press written as in `Action.keys`: modifiers in the order Ctrl, Alt,
 * Shift, then the key. Ctrl stands for Cmd on macOS too.
 */
export function keyCombo(event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string {
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(event.key.length === 1 ? event.key.toUpperCase() : event.key);
  return parts.join("+");
}

const KEY_SYMBOLS: Record<string, string> = { ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };

/** How an action's first key combination is shown in menus and tooltips. */
export function shortcutLabel(action: Action): string | undefined {
  return action.keys?.[0].replace(/Arrow(Up|Down|Left|Right)/, (key) => KEY_SYMBOLS[key]);
}

/** The action's label with its keys, for a tooltip. */
export function tooltip(action: Action): string {
  const keys = shortcutLabel(action);
  return keys ? `${action.label} (${keys})` : action.label;
}

/** Runs the action whose keys are pressed. */
export function useActionKeys(actions: Action[]) {
  const latest = useLatest(actions);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const combo = keyCombo(event);
      // Fields keep their keys, and a focused button its Enter.
      const typing = isTypingTarget(event.target) || (combo === "Enter" && event.target instanceof HTMLButtonElement);
      const action = latest.current.find((a) => a.keys?.includes(combo) && (a.inFields || !typing));
      if (!action) return;
      event.preventDefault();
      if (!action.disabled && (action.repeats || !event.repeat)) action.run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [latest]);
}
