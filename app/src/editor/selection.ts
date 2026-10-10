// Which slides are selected, and the edits that act on all of them: pure,
// so the rules for clicks with Ctrl and Shift are tested without a screen.

import type { ElementId } from "./elements";
import type { Edit } from "./history";
import type { Slide } from "../types";

/**
 * The selected slides. `id` is the one shown on the canvas, kept by id so
 * it stays selected as slides move; `index` is the fallback once it is
 * gone. `others` are selected with it (Ctrl or Shift click), `anchor` is
 * where a Shift click's range starts.
 */
export interface Selection {
  id: string | null;
  index: number;
  element: ElementId;
  others: readonly string[];
  anchor: string | null;
}

export const FIRST: Selection = { id: null, index: 0, element: "slide", others: [], anchor: null };

/** Only the slide at `index` (or the part of it). */
export function selectOne(slides: Slide[], index: number, element: ElementId = "slide"): Selection {
  const id = slides[index]?.id ?? null;
  return { id, index, element, others: [], anchor: id };
}

/** The slides `ids`, the first (at `index`) on the canvas. */
export function selectIds(ids: string[], index: number): Selection {
  const [id = null, ...others] = ids;
  return { id, index, element: "slide", others, anchor: id };
}

/**
 * A click on the slide at `index` in the list: alone; with Ctrl added to
 * the selection or taken out of it; with Shift every slide from the anchor
 * to it (with Ctrl and Shift, added to what is selected).
 */
export function clickSlide(selection: Selection, slides: Slide[], index: number, keys: { ctrl: boolean; shift: boolean }): Selection {
  const clicked = slides[index]?.id;
  if (clicked === undefined) return selection;
  const selected = selectedIds(selection, slides);
  if (keys.shift) {
    const from = slides.findIndex((slide) => slide.id === (selection.anchor ?? selection.id));
    const [low, high] = from < 0 ? [index, index] : [Math.min(from, index), Math.max(from, index)];
    const range = slides.slice(low, high + 1).map((slide) => slide.id);
    const ids = keys.ctrl ? [...selected, ...range] : range;
    return { id: clicked, index, element: "slide", others: unique(ids).filter((id) => id !== clicked), anchor: selection.anchor ?? selection.id };
  }
  if (!keys.ctrl) return selectOne(slides, index);
  if (!selected.includes(clicked)) {
    return { id: clicked, index, element: "slide", others: selected, anchor: clicked };
  }
  const rest = selected.filter((id) => id !== clicked);
  // The last selected slide stays selected.
  if (rest.length === 0) return selection;
  if (clicked !== selection.id) return { ...selection, others: selection.others.filter((id) => id !== clicked) };
  const next = rest[rest.length - 1];
  return { id: next, index: slides.findIndex((slide) => slide.id === next), element: "slide", others: rest.slice(0, -1), anchor: next };
}

/** Every slide, the one on the canvas kept. */
export function selectAll(selection: Selection, slides: Slide[], index: number): Selection {
  const id = slides[index]?.id ?? null;
  return { id, index, element: "slide", others: slides.map((slide) => slide.id).filter((other) => other !== id), anchor: selection.anchor ?? id };
}

/** The ids of the selected slides that still exist, in the quiz's order. */
export function selectedIds(selection: Selection, slides: Slide[]): string[] {
  const ids = new Set([...selection.others, ...(selection.id === null ? [] : [selection.id])]);
  return slides.filter((slide) => ids.has(slide.id)).map((slide) => slide.id);
}

/** The positions of `ids` in `slides`, in order. */
export function positions(slides: Slide[], ids: readonly string[]): number[] {
  return slides.flatMap((slide, i) => (ids.includes(slide.id) ? [i] : []));
}

/** Removes the slides at `indices`, as one change. */
export function removeSlides(indices: number[]): Edit {
  return { type: "batch", edits: [...indices].sort((a, b) => b - a).map((index) => ({ type: "removeSlide", index })) };
}

/** Inserts `inserted` at `at` in order, as one change. */
export function insertSlides(inserted: Slide[], at: number): Edit {
  return { type: "batch", edits: inserted.map((slide, i) => ({ type: "insertSlide", index: at + i, slide })) };
}

/**
 * Moves the slides `ids` together into the gap before slide `gap` (the
 * end when it is the count), in their order, as one change; null when
 * they are there already.
 */
export function moveToGap(slides: Slide[], ids: readonly string[], gap: number): Edit | null {
  const order = slides.map((slide) => slide.id);
  const moving = order.filter((id) => ids.includes(id));
  const staying = order.filter((id) => !ids.includes(id));
  // Where the first of them goes once they are out of the list.
  const start = gap - order.slice(0, gap).filter((id) => ids.includes(id)).length;
  const wanted = [...staying.slice(0, start), ...moving, ...staying.slice(start)];
  // Front to back, each place gets its slide from further down.
  const edits: Edit[] = [];
  wanted.forEach((id, to) => {
    const from = order.indexOf(id);
    if (from === to) return;
    order.splice(from, 1);
    order.splice(to, 0, id);
    edits.push({ type: "moveSlide", from, to });
  });
  return edits.length > 0 ? { type: "batch", edits } : null;
}

/** Moves the slides `ids` one place up or down each, as one change; null when one is at the end already. */
export function moveByOne(slides: Slide[], ids: readonly string[], direction: -1 | 1): Edit | null {
  const indices = positions(slides, ids);
  if (indices.length === 0 || (direction < 0 ? indices[0] === 0 : indices[indices.length - 1] === slides.length - 1)) return null;
  const ordered = direction < 0 ? indices : [...indices].reverse();
  return { type: "batch", edits: ordered.map((from) => ({ type: "moveSlide", from, to: from + direction })) };
}

function unique(ids: string[]): string[] {
  return [...new Set(ids)];
}
