// The editor's context menus: what a right click on a thumbnail, the
// slide list, a part of the slide on the canvas or a category offers. They
// list the actions the ribbon and the keys run (actions.ts).

import type { Translate } from "../i18n";
import { isCorrect } from "../quiz";
import type { Slide } from "../types";
import type { Action, Actions } from "./actions";
import { answerIndex, isMediaElement, type ElementId } from "./elements";
import { toggleCorrect } from "./slides";
import type { CategoryActions } from "./useCategories";
import type { SlideList } from "./useSlideList";

export type MenuTarget = { kind: "thumbnail" } | { kind: "list" } | { kind: "canvas"; element: ElementId } | { kind: "category" };

export type MenuItem = Action | "separator";

export interface MenuContext {
  actions: Actions;
  categoryActions: CategoryActions;
  /** The slide on the canvas. */
  slide: Slide | null;
  update: SlideList["update"];
  /** Opens the gallery of new slides at a point. */
  openGallery: (x: number, y: number) => void;
}

/** Leaves out what cannot be done now, where a menu offers only what can. */
const available = (actions: Action[]) => actions.filter((action) => !action.disabled);

/** What a right click on `target`, at (`x`, `y`), offers. */
export function menuItems(t: Translate, target: MenuTarget, x: number, y: number, context: MenuContext): MenuItem[] {
  const { actions, slide } = context;
  if (target.kind === "category") {
    const c = context.categoryActions;
    return [c.newCategory, c.deleteCategory, c.reapplyCategory, "separator", c.categoryBackground, ...available([c.toggleCategoryBackground, c.removeCategoryBackground])];
  }
  const galleryHere = { ...actions.newSlide, run: () => context.openGallery(x, y) };
  if (target.kind === "list") return [actions.paste, galleryHere];
  if (target.kind === "thumbnail") {
    return [
      actions.cut,
      actions.copy,
      actions.paste,
      actions.selectAll,
      "separator",
      galleryHere,
      actions.duplicate,
      actions.delete,
      "separator",
      actions.moveUp,
      actions.moveDown,
      "separator",
      actions.edit,
      actions.testRunHere,
    ];
  }
  const answer = answerIndex(target.element);
  const items: MenuItem[] = [actions.edit];
  if (answer !== null && slide?.type === "question") {
    items.push({
      label: t("correctAnswer"),
      checked: isCorrect(slide, answer),
      disabled: toggleCorrect(slide, answer) === slide,
      run: () => context.update([slide.id], (s) => (s.type === "question" ? toggleCorrect(s, answer) : s)),
    });
  }
  if (isMediaElement(target.element)) items.push(actions.changeMedia, actions.removeMedia);
  if (target.element === "sound") items.push("separator", actions.trimSound, actions.normalizeSound);
  if (target.element === "video") items.push("separator", actions.trimVideo);
  if (target.element === "slide") items.push("separator", actions.insertBackground, ...available([actions.toggleBackground, actions.removeBackground]));
  return [...items, "separator", actions.cut, actions.copy, actions.paste];
}
