import type { Button } from "./types";

/** Buzzer buttons in the order the UI lists them. */
export const BUTTONS: readonly Button[] = ["a", "b", "c", "d", "big"];

/** Answer letters, one per answer button (`MAX_ANSWERS`). */
export const ANSWER_LETTERS = ["A", "B", "C", "D"] as const;

/** How a button is written on screen: its letter, or a dot for the big button. */
export function buttonLabel(button: Button): string {
  return button === "big" ? "●" : button.toUpperCase();
}
