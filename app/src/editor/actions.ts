// What the editor can do, in one place: the ribbon, the context menus and
// the keyboard all run the same actions.

import { useEffect, type ReactNode } from "react";
import { useLatest } from "../hooks";
import { isFieldTarget, keyCombo } from "../keyboard";

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

/** What is done with the slides as a list (`useSlideList`). */
export type SlideListActionId = "cut" | "copy" | "paste" | "delete" | "duplicate" | "moveUp" | "moveDown" | "selectAll" | "previous" | "next" | "first" | "last";

/** What is done with the slides' media (`useSlideMedia`). */
export type SlideMediaActionId =
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
  | "removeBackground";

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
  | SlideListActionId
  | "edit"
  | "newSlide"
  | "addQuestion"
  | SlideMediaActionId
  | "graphical"
  | "table"
  | "categories"
  | "saveAsCategory"
  | "properties"
  | "findProperty"
  | "importSheet"
  | "sheetTemplate"
  | "exportSheet"
  | "notes"
  | "testRun"
  | "testRunHere";

export type Actions = Record<ActionId, Action>;

const KEY_SYMBOLS: Record<string, string> = { ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Escape: "Esc" };

/** How an action's first key combination is shown in menus and tooltips. */
export function shortcutLabel(action: Pick<Action, "keys">): string | undefined {
  return action.keys?.[0].replace(/Arrow(Up|Down|Left|Right)|Escape/, (key) => KEY_SYMBOLS[key]);
}

/** The action's label with its keys, for a tooltip. */
export function tooltip(action: Pick<Action, "label" | "keys">): string {
  const keys = shortcutLabel(action);
  return keys ? `${action.label} (${keys})` : action.label;
}

/** The keys that delete what is selected. */
const DELETE_KEYS = new Set(["Delete", "Backspace"]);

/** Where Delete and Backspace belong to the control used last (a switch, a button, a property, a table cell), so they never delete a slide. */
const OWNS_DELETE = "button, [role=switch], [role=tab], [role=menuitem], .properties-panel, .slide-table";

/**
 * Whether the key `combo` pressed on `target` is the target's own: a field
 * keeps its keys, a focused button its Enter, and a control used last its
 * Delete and Backspace.
 */
export function keyBelongsTo(target: EventTarget | null, combo: string): boolean {
  if (isFieldTarget(target)) return true;
  if (combo === "Enter") return target instanceof HTMLButtonElement;
  return DELETE_KEYS.has(combo) && target instanceof Element && target.closest(OWNS_DELETE) !== null;
}

/** Runs the action whose keys are pressed. */
export function useActionKeys(actions: Action[]) {
  const latest = useLatest(actions);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const combo = keyCombo(event);
      const typing = keyBelongsTo(event.target, combo);
      const action = latest.current.find((a) => a.keys?.includes(combo) && (a.inFields || !typing));
      if (!action) return;
      event.preventDefault();
      if (!action.disabled && (action.repeats || !event.repeat)) action.run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [latest]);
}
