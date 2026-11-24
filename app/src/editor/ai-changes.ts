// The changes the AI assistant's model made to the quiz (`AiChanges`), as
// the editor's edits: one batch, so one undo step takes them all back.

import type { Category, Quiz, QuizChange, Slide } from "../types";
import { applyEdit, type Edit } from "./history";
import { formatImported, withFreshIds } from "./slides";

/** What the changes did, for the note in the panel. */
export interface ChangeCounts {
  added: number;
  changed: number;
  removed: number;
  moved: number;
  /** Changes of the quiz's own fields (title, theme, settings, sounds, welcome screen, categories). */
  quiz: number;
  /** Changes not made because the quizmaster changed the same slide or field while the model wrote. */
  skipped: number;
}

export interface ChangeEdits {
  edit: Edit;
  counts: ChangeCounts;
  /** The ids of the slides added, in order, to select them. */
  added: string[];
}

/**
 * The edits that make `changes` in `quiz`, as one batch. The model wrote
 * them for `asked`, the quiz as it was when it was asked: a slide or a
 * field of the quiz that has changed since keeps the quizmaster's change
 * and the model's is skipped. Slides are found by id as the quiz has them
 * now: a change of a slide deleted meanwhile does nothing. Added slides get ids no slide has (`withFreshIds`) and the
 * look of their category (`formatImported`). The categories change first,
 * so a slide added in the same reply finds its new category.
 */
export function changeEdits(quiz: Quiz, asked: Quiz, changes: readonly QuizChange[], random?: () => string): ChangeEdits {
  const edits: Edit[] = [];
  const counts: ChangeCounts = { added: 0, changed: 0, removed: 0, moved: 0, quiz: 0, skipped: 0 };
  const added: string[] = [];
  let now = quiz;
  const push = (edit: Edit) => {
    const next = applyEdit(now, edit);
    if (next === now) return false;
    edits.push(edit);
    now = next;
    return true;
  };
  const indexOf = (id: string) => now.slides.findIndex((slide) => slide.id === id);
  // Whether the quizmaster changed a value since the model was asked: then it is theirs.
  const changedSince = (then: unknown, current: unknown) => {
    const changed = JSON.stringify(then) !== JSON.stringify(current);
    if (changed) counts.skipped++;
    return changed;
  };
  // After the slide `after`: none is the start, one gone meanwhile the end.
  const placeAfter = (after: string | null) => (after === null ? 0 : indexOf(after) < 0 ? now.slides.length : indexOf(after) + 1);
  const ordered = [...changes.filter((c) => c.type === "setCategories"), ...changes.filter((c) => c.type !== "setCategories")];
  for (const change of ordered) {
    switch (change.type) {
      case "insertSlides": {
        let at = placeAfter(change.after);
        for (const slide of withFreshIds(now, change.slides, random).map((s) => formatImported(s, now.categories ?? []))) {
          if (push({ type: "insertSlide", index: at, slide })) {
            added.push(slide.id);
            counts.added++;
            at++;
          }
        }
        break;
      }
      case "replaceSlide": {
        const before = now.slides[indexOf(change.slide.id)];
        if (!before || changedSince(asked.slides.find((s) => s.id === before.id), quiz.slides.find((s) => s.id === before.id))) break;
        // A slide that takes on another category takes on its look.
        const slide: Slide = change.slide.category !== before.category ? formatImported(change.slide, now.categories ?? []) : change.slide;
        if (push({ type: "updateSlide", id: slide.id, update: () => slide })) counts.changed++;
        break;
      }
      case "removeSlide":
        if (push({ type: "removeSlide", index: indexOf(change.id) })) counts.removed++;
        break;
      case "moveSlide": {
        const from = indexOf(change.id);
        if (from < 0) break;
        const after = placeAfter(change.after);
        // Taken out first: a slide that moves down lands one before where it was asked to.
        const to = after > from ? after - 1 : after;
        if (push({ type: "moveSlide", from, to })) counts.moved++;
        break;
      }
      case "setTitle":
        if (!changedSince(asked.title, quiz.title) && push({ type: "setTitle", title: change.title })) counts.quiz++;
        break;
      case "setTheme":
        if (!changedSince(asked.theme, quiz.theme) && push({ type: "setTheme", theme: change.theme })) counts.quiz++;
        break;
      case "setSettings":
        if (!changedSince(asked.settings, quiz.settings) && push({ type: "setSettings", settings: change.settings })) counts.quiz++;
        break;
      case "setSounds":
        if (!changedSince(asked.sounds, quiz.sounds) && push({ type: "setSounds", sounds: change.sounds })) counts.quiz++;
        break;
      case "setWelcome":
        if (!changedSince(asked.welcome, quiz.welcome) && push({ type: "setWelcome", welcome: change.welcome })) counts.quiz++;
        break;
      case "setCategories":
        if (!changedSince(asked.categories, quiz.categories) && categoryEdits(now.categories ?? [], change.categories).filter(push).length > 0) counts.quiz++;
        break;
    }
  }
  return { edit: { type: "batch", edits }, counts, added };
}

/** The edits that make the categories `before` into `after`, by id. */
function categoryEdits(before: readonly Category[], after: readonly Category[]): Edit[] {
  const kept = new Set(after.map((category) => category.id));
  const had = new Map(before.map((category) => [category.id, category]));
  const removed: Edit[] = before.filter((category) => !kept.has(category.id)).map((category) => ({ type: "removeCategory", id: category.id }));
  let index = before.length - removed.length;
  const changed: Edit[] = after.flatMap((category): Edit[] => {
    const old = had.get(category.id);
    if (!old) return [{ type: "insertCategory", index: index++, category }];
    return JSON.stringify(old) === JSON.stringify(category) ? [] : [{ type: "updateCategory", id: category.id, update: () => category }];
  });
  return [...removed, ...changed];
}

/** How many changes the counts add up to. */
export function changeCount(counts: ChangeCounts): number {
  return counts.added + counts.changed + counts.removed + counts.moved + counts.quiz;
}
