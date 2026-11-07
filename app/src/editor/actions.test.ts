// @vitest-environment jsdom

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { keyBelongsTo, useActionKeys, type Action } from "./actions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** Adds `html` to the page and gives back its first element. */
function element(html: string): HTMLElement {
  const box = document.createElement("div");
  box.innerHTML = html;
  document.body.append(box);
  return box.firstElementChild as HTMLElement;
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("keys on the control used last", () => {
  it("leave Delete to the selection after a click on a ribbon or quick-access button", () => {
    const ribbon = element('<div class="ribbon"><button type="button">Duplicate</button></div>');
    expect(keyBelongsTo(ribbon.querySelector("button"), "Delete")).toBe(false);
    expect(keyBelongsTo(ribbon.querySelector("button"), "Backspace")).toBe(false);
    // A focused button still keeps its own Enter.
    expect(keyBelongsTo(ribbon.querySelector("button"), "Enter")).toBe(true);
  });

  it("keep Delete on a switch, an answer's toggle, a menu item, a property and a table cell", () => {
    expect(keyBelongsTo(element('<button role="switch"></button>'), "Delete")).toBe(true);
    expect(keyBelongsTo(element('<span class="answer-toggles"><button aria-pressed="true">A</button></span>').querySelector("button"), "Delete")).toBe(true);
    expect(keyBelongsTo(element('<button role="menuitemcheckbox"></button>'), "Delete")).toBe(true);
    expect(keyBelongsTo(element('<div class="properties-panel"><button>Copy</button></div>').querySelector("button"), "Backspace")).toBe(true);
    expect(keyBelongsTo(element('<table class="slide-table"><tr><td tabindex="0"></td></tr></table>').querySelector("td"), "Delete")).toBe(true);
    expect(keyBelongsTo(element("<input />"), "Delete")).toBe(true);
  });
});

function Keys({ actions }: { actions: Action[] }) {
  useActionKeys(actions);
  return null;
}

describe("an action that takes the quiz as it is", () => {
  let root: Root;
  beforeEach(() => {
    root = createRoot(element("<div></div>"));
  });
  afterEach(() => act(() => root.unmount()));

  it("applies the field's text before it decides, and gives the field its focus back", async () => {
    const run = vi.fn();
    // Saved as far as the editor knows: Save is off until the field's text is applied.
    const save = (disabled: boolean): Action => ({ label: "Save", keys: ["Ctrl+S"], inFields: true, appliesField: true, disabled, run });
    act(() => root.render(createElement(Keys, { actions: [save(true)] })));
    const field = element("<input />") as HTMLInputElement;
    field.addEventListener("blur", () => act(() => root.render(createElement(Keys, { actions: [save(false)] }))));
    field.focus();
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "s", ctrlKey: true, bubbles: true }));
    expect(run).not.toHaveBeenCalled();
    await act(() => new Promise((resolve) => setTimeout(resolve)));
    expect(run).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(field);
  });

  it("stays off when the field changed nothing", async () => {
    const run = vi.fn();
    act(() => root.render(createElement(Keys, { actions: [{ label: "Save", keys: ["Ctrl+S"], inFields: true, appliesField: true, disabled: true, run }] })));
    const field = element("<input />");
    field.focus();
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "s", ctrlKey: true, bubbles: true }));
    await act(() => new Promise((resolve) => setTimeout(resolve)));
    expect(run).not.toHaveBeenCalled();
  });
});
