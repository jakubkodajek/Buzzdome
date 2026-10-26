// The quiz being edited, with undo and redo. Pure: time and new slide ids
// come in with the actions, and slide updates are pure functions.

import { sameSounds } from "../audio/library";
import { settingsOf, soundsOf } from "../quiz";
import type { Category, Quiz, QuizSettings, QuizSounds, Slide } from "../types";
import { forgetCategory, formatLike } from "./slides";

export type Edit =
  | { type: "setTitle"; title: string }
  /** The look of the quiz's screens: a theme id (`Quiz.theme`). */
  | { type: "setTheme"; theme: string }
  /** How the show runs (`Quiz.settings`). */
  | { type: "setSettings"; settings: QuizSettings }
  /** What the show sounds like (`Quiz.sounds`). */
  | { type: "setSounds"; sounds: QuizSounds }
  | { type: "insertSlide"; index: number; slide: Slide }
  /** Applies `update` to the slide with this id as it is when the edit lands. */
  | { type: "updateSlide"; id: string; update: (slide: Slide) => Slide }
  | { type: "removeSlide"; index: number }
  | { type: "moveSlide"; from: number; to: number }
  | { type: "insertCategory"; index: number; category: Category }
  /** Applies `update` to the category with this id as it is when the edit lands. */
  | { type: "updateCategory"; id: string; update: (category: Category) => Category }
  /** Removes the category; the slides that took it on forget it and keep their look. */
  | { type: "removeCategory"; id: string }
  /** Gives the slides `slides` the look of the category `id` as it is when the edit lands (`formatLike`). */
  | { type: "applyCategory"; id: string; slides: string[] }
  /** Several edits as one undo step, applied in order. */
  | { type: "batch"; edits: Edit[] };

export type HistoryAction =
  /** `merge` joins quick changes with the same key (typing in one field) into one undo step. */
  | { type: "edit"; edit: Edit; merge?: string; at: number }
  | { type: "undo" }
  | { type: "redo" };

export interface History {
  past: Quiz[];
  present: Quiz;
  future: Quiz[];
  /** The last change that may take further typing into its undo step. */
  open: { key: string; at: number } | null;
}

/** Undo steps kept. */
export const UNDO_LIMIT = 200;
/** Changes to one field closer together than this are one undo step. */
export const MERGE_MS = 1500;

export function startHistory(quiz: Quiz): History {
  return { past: [], present: quiz, future: [], open: null };
}

/** The quiz after `edit`; the same object when the edit changes nothing. */
export function applyEdit(quiz: Quiz, edit: Edit): Quiz {
  const count = quiz.slides.length;
  const inRange = (index: number, end = count) => Number.isInteger(index) && index >= 0 && index < end;
  switch (edit.type) {
    case "setTitle":
      return edit.title === quiz.title ? quiz : { ...quiz, title: edit.title };
    case "setTheme":
      return edit.theme === quiz.theme ? quiz : { ...quiz, theme: edit.theme };
    case "setSettings": {
      const settings = settingsOf(quiz);
      const same = (Object.keys(edit.settings) as (keyof QuizSettings)[]).every((key) => edit.settings[key] === settings[key]);
      return same ? quiz : { ...quiz, settings: edit.settings };
    }
    case "setSounds":
      return sameSounds(soundsOf(quiz), edit.sounds) ? quiz : { ...quiz, sounds: edit.sounds };
    case "insertSlide": {
      if (!inRange(edit.index, count + 1)) return quiz;
      const slides = [...quiz.slides];
      slides.splice(edit.index, 0, edit.slide);
      return { ...quiz, slides };
    }
    case "updateSlide": {
      const index = quiz.slides.findIndex((slide) => slide.id === edit.id);
      if (index < 0) return quiz;
      const updated = edit.update(quiz.slides[index]);
      if (updated === quiz.slides[index]) return quiz;
      return { ...quiz, slides: quiz.slides.map((slide, i) => (i === index ? updated : slide)) };
    }
    case "removeSlide":
      if (!inRange(edit.index)) return quiz;
      return { ...quiz, slides: quiz.slides.filter((_, i) => i !== edit.index) };
    case "moveSlide": {
      if (!inRange(edit.from) || !inRange(edit.to) || edit.from === edit.to) return quiz;
      const slides = [...quiz.slides];
      const [moved] = slides.splice(edit.from, 1);
      slides.splice(edit.to, 0, moved);
      return { ...quiz, slides };
    }
    case "insertCategory": {
      const categories = [...(quiz.categories ?? [])];
      // Slides name their category by id: two with one id would be one.
      if (!inRange(edit.index, categories.length + 1) || categories.some((c) => c.id === edit.category.id)) return quiz;
      categories.splice(edit.index, 0, edit.category);
      return { ...quiz, categories };
    }
    case "updateCategory": {
      const categories = quiz.categories ?? [];
      const index = categories.findIndex((category) => category.id === edit.id);
      if (index < 0) return quiz;
      const updated = edit.update(categories[index]);
      if (updated === categories[index]) return quiz;
      return { ...quiz, categories: categories.map((category, i) => (i === index ? updated : category)) };
    }
    case "removeCategory": {
      const categories = quiz.categories ?? [];
      if (!categories.some((category) => category.id === edit.id)) return quiz;
      return {
        ...quiz,
        categories: categories.filter((category) => category.id !== edit.id),
        slides: quiz.slides.map((slide) => (slide.category === edit.id ? forgetCategory(slide) : slide)),
      };
    }
    case "applyCategory": {
      const category = quiz.categories?.find((c) => c.id === edit.id);
      const ids = new Set(edit.slides);
      if (!category || !quiz.slides.some((slide) => ids.has(slide.id))) return quiz;
      return { ...quiz, slides: quiz.slides.map((slide) => (ids.has(slide.id) ? formatLike(slide, category) : slide)) };
    }
    case "batch":
      return edit.edits.reduce(applyEdit, quiz);
  }
}

export function historyReducer(state: History, action: HistoryAction): History {
  switch (action.type) {
    case "edit": {
      const next = applyEdit(state.present, action.edit);
      if (next === state.present) return state;
      const merge =
        action.merge !== undefined && state.open !== null && state.open.key === action.merge && action.at - state.open.at < MERGE_MS;
      return {
        past: merge ? state.past : [...state.past, state.present].slice(-UNDO_LIMIT),
        present: next,
        future: [],
        open: action.merge === undefined ? null : { key: action.merge, at: action.at },
      };
    }
    case "undo": {
      if (state.past.length === 0) return state;
      return {
        past: state.past.slice(0, -1),
        present: state.past[state.past.length - 1],
        future: [state.present, ...state.future],
        open: null,
      };
    }
    case "redo": {
      if (state.future.length === 0) return state;
      return {
        past: [...state.past, state.present],
        present: state.future[0],
        future: state.future.slice(1),
        open: null,
      };
    }
  }
}
