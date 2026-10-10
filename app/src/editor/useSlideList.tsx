// The quiz's slides in the editor as a list: which are selected and the
// part of the one on the canvas, and what is done with them in order:
// the clipboard, moving them, going from one to the next.

import { useState, type Dispatch, type SetStateAction } from "react";
import { useApp } from "../app-context";
import { ArrowDownIcon, ArrowUpIcon, CopyIcon, CutIcon, PasteIcon, SlideIcon, TrashIcon } from "../icons";
import type { Quiz, Slide } from "../types";
import type { Action, SlideListActionId } from "./actions";
import { hasElement, isMediaElement, type ElementId } from "./elements";
import { FIRST, insertSlides, moveByOne, positions, removeSlides, selectAll, selectedIds, selectIds, selectOne, type Selection } from "./selection";
import { withFreshIds } from "./slides";
import type { QuizEditor } from "./useQuizEditor";

export interface SlideList {
  slides: Slide[];
  selection: Selection;
  setSelection: Dispatch<SetStateAction<Selection>>;
  /** The place of the slide on the canvas; -1 without slides. */
  index: number;
  /** The slide on the canvas. */
  slide: Slide | null;
  /** The ids of the selected slides. */
  selected: string[];
  /** The selected slides, the one on the canvas first. */
  selectedSlides: Slide[];
  /** The selected part of the slide on the canvas; the slide when several are selected. */
  element: ElementId;
  /** Selects the slide at `i`, or `part` of it. */
  select: (i: number, part?: ElementId) => void;
  /** Selects a part of the slide on the canvas, which leaves only that slide selected. */
  selectPart: (part: ElementId) => void;
  /** Changes the slides `ids` as one undo step; quick changes to one `field` of them merge. */
  update: (ids: string[], change: (s: Slide) => Slide, field?: string) => void;
  /** Inserts slides at `at` as one change and selects them. */
  insert: (inserted: Slide[], at: number) => void;
  actions: Record<SlideListActionId, Action>;
}

export function useSlideList(editor: QuizEditor, quiz: Quiz | null): SlideList {
  const { t } = useApp();
  const [selection, setSelection] = useState<Selection>(FIRST);
  const [clipboard, setClipboard] = useState<Slide[] | null>(null);
  const slides = quiz?.slides ?? [];
  const found = selection.id === null ? -1 : slides.findIndex((s) => s.id === selection.id);
  const index = slides.length === 0 ? -1 : found >= 0 ? found : Math.min(selection.index, slides.length - 1);
  const slide: Slide | null = slides[index] ?? null;
  const selected = selectedIds(selection, slides);
  const selectedSlides = slide ? [slide, ...slides.filter((s) => s !== slide && selected.includes(s.id))] : [];
  const element: ElementId = slide && selected.length <= 1 && hasElement(slide, selection.element) ? selection.element : "slide";

  const select = (i: number, part: ElementId = "slide") => setSelection(selectOne(slides, i, part));
  const update = (ids: string[], change: (s: Slide) => Slide, field?: string) =>
    editor.edit(
      ids.length === 1 ? { type: "updateSlide", id: ids[0], update: change } : { type: "batch", edits: ids.map((id) => ({ type: "updateSlide", id, update: change })) },
      field === undefined ? undefined : `${ids.join(",")}:${field}`,
    );
  const insert = (inserted: Slide[], at: number) => {
    editor.edit(insertSlides(inserted, at));
    setSelection(selectIds(inserted.map((s) => s.id), at));
  };
  /** Removes the selected slides as one change; the slide after them is selected. */
  const removeSelected = () => {
    editor.edit(removeSlides(positions(slides, selected)));
    setSelection(selectOne(slides, positions(slides, selected)[0] ?? index));
  };

  const noSlide = slide === null;
  const inOrder = slides.filter((s) => selected.includes(s.id));
  const moveUp = moveByOne(slides, selected, -1);
  const moveDown = moveByOne(slides, selected, 1);
  const actions: Record<SlideListActionId, Action> = {
    cut: {
      label: t("cut"),
      icon: <CutIcon />,
      keys: ["Ctrl+X"],
      disabled: noSlide,
      run: () => {
        setClipboard(inOrder);
        removeSelected();
      },
    },
    copy: { label: t("copy"), icon: <CopyIcon />, keys: ["Ctrl+C"], disabled: noSlide, run: () => setClipboard(inOrder) },
    paste: {
      label: t("paste"),
      icon: <PasteIcon />,
      keys: ["Ctrl+V"],
      disabled: !quiz || !clipboard,
      run: () => quiz && clipboard && insert(withFreshIds(quiz, clipboard), index + 1),
    },
    delete: {
      label: t("deleteSlide"),
      icon: <TrashIcon />,
      // Delete removes what is selected: a picture, video or sound, or else the slides.
      keys: isMediaElement(element) ? undefined : ["Delete", "Backspace"],
      disabled: noSlide,
      run: removeSelected,
    },
    duplicate: {
      label: t("duplicate"),
      icon: <SlideIcon />,
      keys: ["Ctrl+D"],
      disabled: noSlide,
      // The copies go after the last selected slide.
      run: () => quiz && insert(withFreshIds(quiz, inOrder), Math.max(...positions(slides, selected)) + 1),
    },
    moveUp: { label: t("moveUp"), icon: <ArrowUpIcon />, keys: ["Ctrl+ArrowUp"], disabled: moveUp === null, run: () => moveUp && editor.edit(moveUp) },
    moveDown: { label: t("moveDown"), icon: <ArrowDownIcon />, keys: ["Ctrl+ArrowDown"], disabled: moveDown === null, run: () => moveDown && editor.edit(moveDown) },
    selectAll: { label: t("selectAllSlides"), keys: ["Ctrl+A"], disabled: slides.length < 2, run: () => setSelection(selectAll(selection, slides, index)) },
    previous: { label: t("previousSlide"), keys: ["ArrowUp", "ArrowLeft", "PageUp"], repeats: true, disabled: index <= 0, run: () => select(index - 1) },
    next: {
      label: t("nextSlide"),
      keys: ["ArrowDown", "ArrowRight", "PageDown"],
      repeats: true,
      disabled: index >= slides.length - 1,
      run: () => select(index + 1),
    },
    first: { label: t("firstSlide"), keys: ["Home"], disabled: index <= 0, run: () => select(0) },
    last: { label: t("lastSlide"), keys: ["End"], disabled: index >= slides.length - 1, run: () => select(slides.length - 1) },
  };

  return {
    slides,
    selection,
    setSelection,
    index,
    slide,
    selected,
    selectedSlides,
    element,
    select,
    selectPart: (part) => setSelection(selectOne(slides, index, part)),
    update,
    insert,
    actions,
  };
}
