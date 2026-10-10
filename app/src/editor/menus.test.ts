import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { QuestionSlide, Slide } from "../types";
import type { Action, Actions } from "./actions";
import { menuItems, type MenuContext, type MenuItem, type MenuTarget } from "./menus";
import { newSlide } from "./slides";
import type { CategoryActions } from "./useCategories";

const t = translator("en");

/** Every action, labelled by its id; the ids in `off` cannot be done now. */
function actionsNamed<T extends Actions | CategoryActions>(off: string[] = []): T {
  return new Proxy({} as T, { get: (_, id) => ({ label: String(id), disabled: off.includes(String(id)), run: () => {} }) });
}

function context(slide: Slide | null, off: string[] = [], changed: Slide[] = [], galleries: [number, number][] = []): MenuContext {
  return {
    actions: actionsNamed<Actions>(off),
    categoryActions: actionsNamed<CategoryActions>(off),
    slide,
    update: (_ids, change) => slide && changed.push(change(slide)),
    openGallery: (x, y) => galleries.push([x, y]),
  };
}

/** A menu as its labels, a separator as "-". */
const labels = (items: MenuItem[]) => items.map((item) => (item === "separator" ? "-" : item.label));

const question = newSlide("question", "q") as QuestionSlide;
const menu = (target: MenuTarget, ctx = context(question)) => labels(menuItems(t, target, 10, 20, ctx));

describe("the editor's context menus", () => {
  it("offer a category's actions, leaving out what cannot be done now", () => {
    expect(menu({ kind: "category" }, context(null, ["removeCategoryBackground"]))).toEqual([
      "newCategory",
      "deleteCategory",
      "reapplyCategory",
      "-",
      "categoryBackground",
      "toggleCategoryBackground",
    ]);
  });

  it("open the gallery of new slides where the list or a thumbnail was clicked", () => {
    const galleries: [number, number][] = [];
    const items = menuItems(t, { kind: "list" }, 10, 20, context(question, [], [], galleries));
    expect(labels(items)).toEqual(["paste", "newSlide"]);
    (items[1] as Action).run();
    expect(galleries).toEqual([[10, 20]]);
    expect(menu({ kind: "thumbnail" })).toContain("newSlide");
  });

  it("toggle an answer right or wrong from its own menu", () => {
    const changed: Slide[] = [];
    const items = menuItems(t, { kind: "canvas", element: "answer-1" }, 0, 0, context(question, [], changed));
    const toggle = items.find((item): item is Action => item !== "separator" && item.label === t("correctAnswer"));
    expect(toggle).toMatchObject({ checked: false, disabled: false });
    toggle?.run();
    expect((changed[0] as QuestionSlide).correct.map((c) => c.answer)).toContain(1);
  });

  it("offer a medium's tools on it, and the background's on the slide", () => {
    expect(menu({ kind: "canvas", element: "sound" })).toEqual(["edit", "changeMedia", "removeMedia", "-", "trimSound", "normalizeSound", "-", "cut", "copy", "paste"]);
    expect(menu({ kind: "canvas", element: "video" })).toContain("trimVideo");
    expect(menu({ kind: "canvas", element: "picture" })).not.toContain("trimVideo");
    expect(menu({ kind: "canvas", element: "slide" }, context(question, ["toggleBackground", "removeBackground"]))).toEqual([
      "edit",
      "-",
      "insertBackground",
      "-",
      "cut",
      "copy",
      "paste",
    ]);
    expect(menu({ kind: "canvas", element: "timer" })).toEqual(["edit", "-", "cut", "copy", "paste"]);
  });
});
