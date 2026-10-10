// Categories (spec QZ-10): named looks of a quiz, each a background and a
// style over the theme. A category is drawn and edited as a sample slide,
// so the slide's own background and look rows serve it too, and its look
// is copied onto slides by `formatLike` (slides.ts).

import type { Category, Quiz, Slide } from "../types";
import { DEFAULT_LAYOUT, slideFromLayout } from "./layouts";
import { freshId, randomId, setHeadline } from "./slides";

/** An id no category of `quiz` has yet. */
export function freshCategoryId(quiz: Quiz, random: () => string = randomId): string {
  return freshId(new Set((quiz.categories ?? []).map((category) => category.id)), random, "c");
}

/** The name as categories are told apart (quiz_core::category::name_key): trimmed, regardless of case. */
function nameKey(name: string): string {
  return name.trim().toLowerCase();
}

/** The first of "`base` 1", "`base` 2"… that no category of `categories` is named. */
export function freshCategoryName(categories: readonly Category[], base: string): string {
  const taken = new Set(categories.map((category) => nameKey(category.name)));
  for (let n = 1; ; n++) {
    const name = `${base} ${n}`;
    if (!taken.has(nameKey(name))) return name;
  }
}

/** A category of the theme's look, or of the look `look` has: its background and style. */
export function newCategory(id: string, name: string, look?: Slide): Category {
  return { id, name, background: look?.background ?? null, style: look?.style };
}

const samples = new WeakMap<Category, Slide>();

/**
 * The category as a question in the default layout that asks its name,
 * with its background and style. The same slide for the same category, so
 * its thumbnail is drawn again only when the category changes.
 */
export function sampleSlide(category: Category): Slide {
  let sample = samples.get(category);
  if (!sample) {
    sample = { ...setHeadline(slideFromLayout(DEFAULT_LAYOUT, category.id), category.name), background: category.background, style: category.style };
    samples.set(category, sample);
  }
  return sample;
}

/** The category with the look of `slide`: its background and style. The same object when they are the category's already. */
export function withLook(category: Category, slide: Slide): Category {
  if (slide.background === category.background && slide.style === category.style) return category;
  return { ...category, background: slide.background, style: slide.style };
}

/** The ids of the slides that took the category on, in order. */
export function slidesOf(quiz: Quiz, id: string): string[] {
  return quiz.slides.filter((slide) => slide.category === id).map((slide) => slide.id);
}
