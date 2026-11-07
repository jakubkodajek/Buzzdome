import type { BuzzerId } from "../types";

/** How a row was clicked: with Ctrl (Cmd) to add or take away one, with Shift for a range. */
export interface ClickKeys {
  toggle: boolean;
  range: boolean;
}

/**
 * The players selected after a click on `buzzer`'s row, in the table's
 * order `rows`: a plain click selects that row alone, or nothing when it
 * was the only one selected; Ctrl adds or takes away the row; Shift adds
 * the rows from the `anchor` (the row clicked last without Shift).
 */
export function clickSelection(selected: ReadonlySet<BuzzerId>, rows: readonly BuzzerId[], buzzer: BuzzerId, keys: ClickKeys, anchor: BuzzerId | null): Set<BuzzerId> {
  if (keys.range && anchor !== null && rows.includes(anchor)) {
    const next = new Set(keys.toggle ? selected : []);
    const [from, to] = [rows.indexOf(anchor), rows.indexOf(buzzer)].sort((a, b) => a - b);
    rows.slice(from, to + 1).forEach((id) => next.add(id));
    return next;
  }
  if (keys.toggle) {
    const next = new Set(selected);
    if (!next.delete(buzzer)) next.add(buzzer);
    return next;
  }
  return selected.size === 1 && selected.has(buzzer) ? new Set() : new Set([buzzer]);
}

/** The points typed for the selected players: a whole number other than 0, else null. */
export function typedDelta(text: string): number | null {
  const n = text.trim() === "" ? NaN : Number(text);
  return Number.isInteger(n) && n !== 0 ? n : null;
}

/** The buzzer number typed for a new player: a whole number from 1, else null. */
export function typedBuzzer(text: string): BuzzerId | null {
  const n = text.trim() === "" ? NaN : Number(text);
  return Number.isInteger(n) && n >= 1 ? n : null;
}
