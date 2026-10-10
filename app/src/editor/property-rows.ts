// What the properties panel does with rows, apart from drawing them: which
// rows several slides share, searching, dragging a number, copying and
// pasting a value. Pure, so it is tested without a screen.

import type { ReactNode } from "react";
import { isColor, readColor } from "../theme/color";
import type { Color, Gradient, TextStyle } from "../types";
import type { Action } from "./actions";
import { isGradient, readGradient } from "./gradients";

/** A text's font as one property: the font, its size and the B, I and U buttons. */
export type FontValue = Pick<TextStyle, "font" | "size" | "bold" | "italic" | "underline">;

/** A row's value and how to set it, for Reset, Copy and Paste and to compare slides. */
export type RowValue =
  | { kind: "number"; value: number; initial?: number; min: number; max: number; step: number; set: (value: number) => void }
  | { kind: "flag"; value: boolean; initial?: boolean; set: (value: boolean) => void }
  | { kind: "choice"; value: string | number; initial?: string | number; options: (string | number)[]; set: (value: string | number) => void }
  | { kind: "media"; value: string | null; initial?: null; accepts: (file: string) => boolean; set: (file: string | null) => void }
  /** Null: the theme's colour (or, where there is none, no colour). */
  | { kind: "color"; value: Color | null; initial?: Color | null; set: (color: Color | null) => void }
  /** Null: no gradient, the theme's look. */
  | { kind: "gradient"; value: Gradient | null; initial?: null; set: (gradient: Gradient | null) => void }
  | { kind: "font"; value: FontValue; initial?: FontValue; set: (font: FontValue) => void };

export interface PropertyRow {
  /** Unique among a slide's rows and the same for the same property on every slide. */
  id: string;
  label: string;
  description: string;
  control: ReactNode;
  /** The control spans the panel's width, under its label. */
  wide?: boolean;
  /** Seldom needed: shown after the section's More options. */
  rare?: boolean;
  /** Without one, the row is the selected slide's alone and not offered for several slides. */
  value?: RowValue;
  /** The rows of its parts (a gradient's From, To and Type), shown under it once its name is clicked; the row sums them up. */
  parts?: PropertyRow[];
  /** More entries for the row's context menu. */
  menu?: Action[];
}

/** The row and the rows of its parts, in order. */
export function withParts(rows: PropertyRow[]): PropertyRow[] {
  return rows.flatMap((row) => [row, ...(row.parts ?? [])]);
}

/** Which of the first slide's rows (and their parts) every slide has, and those whose values differ. */
export function commonRows(perSlide: PropertyRow[][]): { shown: Set<string>; mixed: Set<string> } {
  const [first = [], ...others] = perSlide.map(withParts);
  const shown = new Set<string>();
  const mixed = new Set<string>();
  for (const row of first) {
    const value = row.value;
    if (!value) continue;
    const matching = others.map((rows) => rows.find((other) => other.id === row.id && other.value?.kind === value.kind)?.value);
    if (matching.some((other) => !other)) continue;
    shown.add(row.id);
    if (matching.some((other) => other && !sameValue(other.value, value.value))) mixed.add(row.id);
  }
  return { shown, mixed };
}

/** Whether two values of one kind of row are the same: a gradient or a font by what it holds. */
export function sameValue(a: RowValue["value"], b: RowValue["value"]): boolean {
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return a === b;
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  return [...keys].every((key) => (a as Record<string, unknown>)[key] === (b as Record<string, unknown>)[key]);
}

/** A part of a text, and whether it matches the search. */
export interface TextPart {
  text: string;
  match: boolean;
}

/** Lower case without accents, so "cas" finds "Čas": each character of `text` alone, so positions stay. */
function folded(text: string): string[] {
  return [...text].map((char) => char.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase());
}

/** `text` split around the first match of `needle`, ignoring case and accents; null when it does not match. */
export function findMatch(text: string, needle: string): TextPart[] | null {
  const chars = [...text];
  const parts = folded(text);
  const wanted = folded(needle.trim()).join("");
  if (wanted === "") return null;
  // Where each character's folded form starts in the folded text.
  const starts: number[] = [];
  let length = 0;
  for (const part of parts) {
    starts.push(length);
    length += part.length;
  }
  const at = parts.join("").indexOf(wanted);
  if (at < 0) return null;
  // The characters whose folded forms the match covers.
  const end = at + wanted.length;
  const covered = starts.flatMap((start, i) => (start + parts[i].length > at && start < end ? [i] : []));
  const [first, last] = [covered[0], covered[covered.length - 1]];
  return [
    { text: chars.slice(0, first).join(""), match: false },
    { text: chars.slice(first, last + 1).join(""), match: true },
    { text: chars.slice(last + 1).join(""), match: false },
  ].filter((part) => part.text !== "");
}

/** Whether the search finds the row by its label or description. */
export function rowMatches(row: Pick<PropertyRow, "label" | "description">, needle: string): boolean {
  return findMatch(row.label, needle) !== null || findMatch(row.description, needle) !== null;
}

/** Pixels the pointer moves for one step when a number is dragged by its label. */
export const DRAG_PX_PER_STEP = 4;

/**
 * The value of a number dragged by its label `dx` pixels from where it was
 * `start`: a step per few pixels, ten steps with Shift, a quarter as fast
 * with Alt, within the number's limits and on its steps.
 */
export function draggedValue(start: number, dx: number, number: { min: number; max: number; step: number }, keys: { shift: boolean; alt: boolean }): number {
  const pxPerStep = keys.alt ? DRAG_PX_PER_STEP * 4 : DRAG_PX_PER_STEP;
  const steps = Math.trunc(dx / pxPerStep) * (keys.shift ? 10 : 1);
  return roundedTo(Math.min(number.max, Math.max(number.min, start + steps * number.step)), number.step);
}

/** `value` on the grid of `step`, without float noise (0.1 + 0.2 shows as 0.3). */
function roundedTo(value: number, step: number): number {
  const decimals = Math.max(0, -Math.floor(Math.log10(step)));
  return Number((Math.round(value / step) * step).toFixed(decimals));
}

/** A value copied from a row, to paste into this row or another of its kind. */
export interface CopiedValue {
  id: string;
  kind: RowValue["kind"];
  value: RowValue["value"];
}

export function copiedValue(row: PropertyRow): CopiedValue | null {
  return row.value ? { id: row.id, kind: row.value.kind, value: row.value.value } : null;
}

/**
 * What pasting `copied` into `row` does, or null when it cannot: a number
 * goes into any number (brought within its limits and onto its steps), a
 * switch into any switch, a file into a row that takes its kind, a colour,
 * a gradient or a font into any of its kind, a choice only into the same
 * property.
 */
export function pasteAction(row: PropertyRow, copied: CopiedValue): (() => void) | null {
  const target = row.value;
  const value = copied.value;
  if (!target || target.kind !== copied.kind) return null;
  switch (target.kind) {
    case "number":
      return typeof value === "number" ? () => target.set(roundedTo(Math.min(target.max, Math.max(target.min, value)), target.step)) : null;
    case "flag":
      return typeof value === "boolean" ? () => target.set(value) : null;
    case "choice":
      return copied.id === row.id && (typeof value === "string" || typeof value === "number") && target.options.includes(value) ? () => target.set(value) : null;
    case "media":
      return value === null || (typeof value === "string" && target.accepts(value)) ? () => target.set(value) : null;
    case "color":
      return value === null || (typeof value === "string" && isColor(value)) ? () => target.set(value) : null;
    case "gradient":
      return value === null || isGradient(value) ? () => target.set(value) : null;
    case "font":
      return typeof value === "object" && value !== null && !isGradient(value) ? () => target.set(value) : null;
  }
}

/**
 * What pasting text from another program into `row` does, or null when the
 * text is no value for it: a colour as `#1E4A6B` (or as `readColor` takes
 * it), a number, `true` or `false`, one of a choice's options, or a
 * gradient as `valueText` writes it.
 */
export function pasteText(row: PropertyRow, text: string): (() => void) | null {
  const value = row.value;
  if (!value) return null;
  const typed = (kind: RowValue["kind"], pasted: RowValue["value"] | undefined) =>
    pasted === undefined ? null : pasteAction(row, { id: row.id, kind, value: pasted });
  const trimmed = text.trim();
  switch (value.kind) {
    case "color":
      return typed("color", readColor(trimmed) ?? undefined);
    case "number":
      return typed("number", trimmed !== "" && Number.isFinite(Number(trimmed)) ? Number(trimmed) : undefined);
    case "flag":
      return typed("flag", trimmed === "true" ? true : trimmed === "false" ? false : undefined);
    case "choice":
      return typed("choice", value.options.find((option) => String(option) === trimmed));
    case "gradient":
      return typed("gradient", readGradient(trimmed) ?? undefined);
    case "media":
    case "font":
      return null;
  }
}

/** Whether Reset would change the row: it has a default and is not at it. */
export function canReset(row: PropertyRow): boolean {
  const value = row.value;
  return value !== undefined && value.initial !== undefined && !sameValue(value.initial, value.value);
}

/** Sets the row to its default; see `canReset`. */
export function resetValue(row: PropertyRow): void {
  const value = row.value;
  if (!value || value.initial === undefined) return;
  switch (value.kind) {
    case "number":
      value.set(value.initial);
      break;
    case "flag":
      value.set(value.initial);
      break;
    case "choice":
      value.set(value.initial);
      break;
    case "media":
    case "gradient":
      value.set(null);
      break;
    case "color":
      value.set(value.initial);
      break;
    case "font":
      value.set(value.initial);
      break;
  }
}

/** The value as text for other programs: a colour as `#1E4A6B`, a gradient or a font as JSON. */
export function valueText(value: CopiedValue): string {
  if (value.value === null) return "";
  return typeof value.value === "object" ? JSON.stringify(value.value) : String(value.value);
}

/**
 * The rows of every group sorted by label for the classic list, in the
 * language's order; each property once. A label that several properties
 * share (the volume of the video and of the sound) names its group.
 */
export function alphabetical(groups: { name: string; rows: PropertyRow[] }[], locale: string): PropertyRow[] {
  const seen = new Set<string>();
  const rows = groups.flatMap((group) => group.rows.filter((row) => !seen.has(row.id) && seen.add(row.id)).map((row) => ({ row, group: group.name })));
  const count = new Map<string, number>();
  rows.forEach(({ row }) => count.set(row.label, (count.get(row.label) ?? 0) + 1));
  return rows
    .map(({ row, group }) => ((count.get(row.label) ?? 0) > 1 ? { ...row, label: `${row.label} (${group})` } : row))
    .sort((a, b) => a.label.localeCompare(b.label, locale));
}
