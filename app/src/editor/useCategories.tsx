// The Categories view (spec QZ-10): the quiz's categories, the one shown
// and the part of it selected, and what can be done with them. A category
// is edited as its sample slide (categories.ts), so the slide's own
// background and look rows serve it too.

import { useRef, useState } from "react";
import { api } from "../api";
import { useApp, useReport } from "../app-context";
import { MAX_CATEGORY_NAME_CHARS } from "../bindings/Constants";
import { BackgroundIcon, CategoryIcon, PaletteIcon, PlayIcon, StopIcon, TrashIcon } from "../icons";
import { isVideo, type MediaKind } from "../media";
import { NO_CONTEXT } from "../quiz";
import type { Category, Quiz, Slide } from "../types";
import type { Action } from "./actions";
import { freshCategoryId, newCategory, sampleSlide, slidesOf, withLook } from "./categories";
import { hasElement, type ElementId } from "./elements";
import type { SlideEditing } from "./properties";
import { categoryPages, type Page } from "./property-pages";
import { rowBuilders } from "./row-builders";
import { setBackground } from "./slides";
import type { QuizEditor } from "./useQuizEditor";

export type CategoryActionId =
  | "newCategory"
  | "deleteCategory"
  | "reapplyCategory"
  | "previousCategory"
  | "nextCategory"
  | "categoryBackground"
  | "toggleCategoryBackground"
  | "removeCategoryBackground";

export type CategoryActions = Record<CategoryActionId, Action>;

export interface CategoryEditor {
  categories: readonly Category[];
  /** The category shown; null for a quiz without categories. */
  category: Category | null;
  index: number;
  select: (index: number) => void;
  /** The part of the category's sample slide selected on the canvas. */
  element: ElementId;
  selectPart: (element: ElementId) => void;
  /** The category's background video plays on the canvas (it shows its first frame otherwise). */
  playingBackground: boolean;
  /** The properties panel's pages for the category; none without one. */
  pages: Page[];
  /** How many slides took the category shown on. */
  slideCount: number;
  actions: CategoryActions;
  /** Adds a category of the theme's look, or of the look of `look`, after the one shown, and shows it. */
  create: (look?: Slide) => void;
}

export function useCategories(editor: QuizEditor, quiz: Quiz | null): CategoryEditor {
  const { t, notify } = useApp();
  const report = useReport();
  const [selection, setSelection] = useState<{ id: string | null; index: number; element: ElementId }>({ id: null, index: 0, element: "slide" });
  /** A category is being named, so the next waits for it. */
  const creating = useRef(false);
  const [playingBackgroundOf, setPlayingBackgroundOf] = useState<string | null>(null);
  const categories = quiz?.categories ?? [];
  const found = selection.id === null ? -1 : categories.findIndex((c) => c.id === selection.id);
  const index = categories.length === 0 ? -1 : found >= 0 ? found : Math.min(selection.index, categories.length - 1);
  const category: Category | null = categories[index] ?? null;
  const sample = category && sampleSlide(category);
  const element = sample && hasElement(sample, selection.element) ? selection.element : "slide";
  const slides = quiz && category ? slidesOf(quiz, category.id) : [];
  const backgroundVideo = category?.background ? isVideo(category.background.file) : false;
  const playingBackground = backgroundVideo && playingBackgroundOf === category?.id;

  const select = (i: number) => setSelection({ id: categories[i]?.id ?? null, index: i, element: "slide" });
  /** Changes the category `id` as it is when the change lands; quick changes to one `field` are one undo step. */
  const update = (id: string, change: (c: Category) => Category, field?: string) =>
    editor.edit({ type: "updateCategory", id, update: change }, field === undefined ? undefined : `category:${id}:${field}`);
  /** Changes the category `id` as its sample slide. */
  const restyle = (id: string, change: (slide: Slide) => Slide, field?: string) => update(id, (c) => withLook(c, change(sampleSlide(c))), field);
  const chooseMedia = (id: string, kinds: MediaKind[], apply: (slide: Slide, file: string) => Slide) =>
    void editor.pickMedia(kinds).then((file) => file && restyle(id, (slide) => apply(slide, file)));

  const create = (look?: Slide) => {
    // A second click before the first category is in would name and number
    // it from the same quiz, as a twin with the same id.
    if (!quiz || creating.current) return;
    creating.current = true;
    // The backend tells category names apart.
    api
      .editorCategoryName(categories, t("categoryDefaultName"))
      .then((name) => {
        const added = newCategory(freshCategoryId(quiz), name, look);
        editor.edit({ type: "insertCategory", index: index + 1, category: added });
        setSelection({ id: added.id, index: index + 1, element: "slide" });
      }, report)
      .finally(() => (creating.current = false));
  };

  const editing: SlideEditing | null = category &&
    sample && {
      slide: sample,
      context: NO_CONTEXT,
      categories: [],
      update: (change, field) => restyle(category.id, change, field),
      chooseMedia: (kinds, apply) => chooseMedia(category.id, kinds, apply),
      // The sample has no sound or video to trim or measure.
      trimSound: () => {},
      trimVideo: () => {},
      normalizeSound: () => {},
    };
  const name =
    category &&
    rowBuilders(t).row(
      "category.name",
      "propCategoryName",
      "descCategoryName",
      <input
        type="text"
        aria-label={t("propCategoryName")}
        maxLength={MAX_CATEGORY_NAME_CHARS}
        value={category.name}
        onChange={(e) => update(category.id, (c) => ({ ...c, name: e.target.value }), "name")}
      />,
    );

  const actions: CategoryActions = {
    newCategory: { label: t("newCategory"), icon: <CategoryIcon />, disabled: !quiz, run: () => create() },
    deleteCategory: {
      label: t("deleteCategory"),
      icon: <TrashIcon />,
      keys: ["Delete", "Backspace"],
      disabled: !category,
      run: () => {
        if (!category) return;
        editor.edit({ type: "removeCategory", id: category.id });
        setSelection({ id: null, index, element: "slide" });
      },
    },
    reapplyCategory: {
      label: t("reapplyCategory"),
      icon: <PaletteIcon />,
      disabled: slides.length === 0,
      run: () => {
        if (!category) return;
        editor.edit({ type: "applyCategory", id: category.id, slides });
        notify(t("categoryReapplied", { n: slides.length, name: category.name }));
      },
    },
    previousCategory: { label: t("previousCategory"), keys: ["ArrowUp", "ArrowLeft", "PageUp"], repeats: true, disabled: index <= 0, run: () => select(index - 1) },
    nextCategory: {
      label: t("nextCategory"),
      keys: ["ArrowDown", "ArrowRight", "PageDown"],
      repeats: true,
      disabled: index >= categories.length - 1,
      run: () => select(index + 1),
    },
    categoryBackground: {
      label: t("mediaBackground"),
      icon: <BackgroundIcon />,
      disabled: !category,
      run: () => category && chooseMedia(category.id, ["image", "video"], setBackground),
    },
    toggleCategoryBackground: {
      label: t(playingBackground ? "stopBackground" : "playBackground"),
      icon: playingBackground ? <StopIcon /> : <PlayIcon />,
      disabled: !backgroundVideo,
      run: () => category && setPlayingBackgroundOf(playingBackground ? null : category.id),
    },
    removeCategoryBackground: {
      label: t("removeBackground"),
      icon: <TrashIcon />,
      disabled: !category?.background,
      run: () => category && restyle(category.id, (slide) => setBackground(slide, null)),
    },
  };

  return {
    categories,
    category,
    index,
    select,
    element,
    selectPart: (part) => setSelection({ id: category?.id ?? null, index, element: part }),
    playingBackground,
    pages: editing && name ? categoryPages(t, editing, name, element) : [],
    slideCount: slides.length,
    actions,
    create,
  };
}
