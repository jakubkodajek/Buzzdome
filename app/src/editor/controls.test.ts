// @vitest-environment jsdom

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider } from "../app-context";
import { Dialog, Popup } from "./controls";
import { useCurrentItem } from "./Thumbnails";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn(), message: vi.fn() }));
vi.mock("../api", () => ({ api: {} }));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

beforeAll(() => {
  // jsdom lays nothing out: every control counts as shown, and nothing resizes.
  vi.spyOn(HTMLElement.prototype, "getClientRects").mockReturnValue([{}] as unknown as DOMRectList);
  HTMLElement.prototype.scrollIntoView ??= () => undefined;
  globalThis.ResizeObserver ??= class {
    observe() {}
    disconnect() {}
    unobserve() {}
  } as unknown as typeof ResizeObserver;
});

let root: Root;
beforeEach(() => {
  const box = document.createElement("div");
  document.body.append(box);
  root = createRoot(box);
});
afterEach(() => {
  act(() => root.unmount());
  document.body.innerHTML = "";
});

const button = (label: string, tabIndex?: number) => createElement("button", { type: "button", key: label, tabIndex }, label);
const named = (label: string) => [...document.querySelectorAll("button")].find((b) => b.textContent === label) as HTMLElement;
function tab(from: HTMLElement, shiftKey = false) {
  act(() => void from.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true, cancelable: true })));
}

describe("Popup", () => {
  it("keeps Tab going round its own controls, past those taken off the Tab order", () => {
    const behind = document.createElement("button");
    document.body.append(behind);
    behind.focus();
    act(() => root.render(createElement(Popup, { x: 0, y: 0, onClose: () => undefined, className: "menu", role: "menu", children: [button("A"), button("B"), button("Off", -1)] })));
    named("B").focus();
    tab(named("B"));
    expect(document.activeElement).toBe(named("A"));
    tab(named("A"), true);
    expect(document.activeElement).toBe(named("B"));
  });
});

describe("Dialog", () => {
  const dialog = (onClose: () => void, staysOpen: boolean) =>
    createElement(AppProvider, { show: () => undefined, children: createElement(Dialog, { title: "Quiz", onClose, footer: null, staysOpen, children: button("OK") }) });

  it("closes on a click beside it, unless it stays open", () => {
    for (const staysOpen of [false, true]) {
      const onClose = vi.fn();
      act(() => root.render(dialog(onClose, staysOpen)));
      const backdrop = document.querySelector(".dialog-backdrop") as HTMLElement;
      act(() => void backdrop.dispatchEvent(new MouseEvent("mousedown", { bubbles: true })));
      expect(onClose).toHaveBeenCalledTimes(staysOpen ? 0 : 1);
    }
  });
});

function List({ items, current }: { items: string[]; current: number }) {
  const item = useCurrentItem(current);
  return createElement(
    "ol",
    null,
    items.map((id, i) => createElement("li", { key: id, ...item(i) }, id)),
  );
}

describe("useCurrentItem", () => {
  it("gives the focus to the current item when the focused one is deleted", () => {
    act(() => root.render(createElement(List, { items: ["a", "b", "c"], current: 1 })));
    const b = document.querySelectorAll("li")[1];
    act(() => b.focus());
    act(() => root.render(createElement(List, { items: ["a", "c"], current: 1 })));
    expect(document.activeElement?.textContent).toBe("c");
  });

  it("leaves the focus where it is when the list does not have it", () => {
    const elsewhere = document.createElement("button");
    document.body.append(elsewhere);
    act(() => root.render(createElement(List, { items: ["a", "b"], current: 0 })));
    elsewhere.focus();
    act(() => root.render(createElement(List, { items: ["b"], current: 0 })));
    expect(document.activeElement).toBe(elsewhere);
  });
});
