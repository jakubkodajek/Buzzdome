import { ANSWER_BUTTONS } from "./bindings/Constants";
import type { Button } from "./types";

/** Buzzer buttons in the order the UI lists them. */
export const BUTTONS: readonly Button[] = [...ANSWER_BUTTONS, "big"];

/**
 * Answer letters (`MAX_ANSWERS`). The first `MAX_EVERYONE_ANSWERS` are the
 * buzzer's answer buttons; E and F are only read out (fastest finger).
 */
export const ANSWER_LETTERS = ["A", "B", "C", "D", "E", "F"] as const;

/** Which answer a button gives, from 0, or null for the big button (`Button::answer_index`). */
export function answerIndex(button: Button): number | null {
  const index = (ANSWER_BUTTONS as readonly Button[]).indexOf(button);
  return index < 0 ? null : index;
}

/** How a button is written on screen: its letter, or a dot for the big button. */
export function buttonLabel(button: Button): string {
  return button === "big" ? "●" : button.toUpperCase();
}
