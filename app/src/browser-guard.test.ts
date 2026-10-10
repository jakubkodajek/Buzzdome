import { describe, expect, it } from "vitest";
import { installBrowserGuard, isBrowserKey } from "./browser-guard";

const key = (key: string, mods: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>> = {}, target: unknown = { tagName: "DIV" }) => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  target: target as EventTarget,
  ...mods,
});

type Listener = (event: GuardEvent) => void;

/** As much of a DOM event as the guard reads, and whether it was prevented or stopped. */
interface GuardEvent {
  target: unknown;
  key?: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  prevented: boolean;
  stopped: boolean;
  preventDefault: () => void;
  stopPropagation: () => void;
}

/**
 * A window with the guard installed, and a way to fire an event at a child
 * of it the way the DOM does: the window's capture listeners first, then the
 * child's own handler, then the window's bubble listeners unless the child
 * stopped the event, as React's handlers do at the root.
 */
function guardedWindow() {
  const listeners: { type: string; listener: Listener; capture: boolean }[] = [];
  installBrowserGuard({
    addEventListener: ((type: string, listener: Listener, options?: boolean | AddEventListenerOptions) => {
      listeners.push({ type, listener, capture: typeof options === "boolean" ? options : options?.capture === true });
    }) as never,
  });
  return (type: string, fields: Partial<GuardEvent> & { target: unknown }, child?: (event: GuardEvent) => void) => {
    const event: GuardEvent = {
      prevented: false,
      stopped: false,
      preventDefault: () => (event.prevented = true),
      stopPropagation: () => (event.stopped = true),
      ...fields,
    };
    const run = (capture: boolean) => listeners.filter((l) => l.type === type && l.capture === capture).forEach((l) => l.listener(event));
    run(true);
    child?.(event);
    if (!event.stopped) run(false);
    return event.prevented;
  };
}

describe("browser guard", () => {
  it("takes the webview's reload, navigation, find and print keys", () => {
    for (const press of [
      key("F5"),
      key("r", { ctrlKey: true }),
      key("R", { ctrlKey: true, shiftKey: true }),
      key("ArrowLeft", { altKey: true }),
      key("BrowserBack"),
      key("f", { ctrlKey: true }),
      key("p", { metaKey: true }),
    ]) {
      expect(isBrowserKey(press), JSON.stringify(press)).toBe(true);
    }
  });

  it("leaves word moves to text fields, where Option+arrows move the caret on macOS", () => {
    const field = { tagName: "INPUT", type: "text" };
    expect(isBrowserKey(key("ArrowLeft", { altKey: true }, field))).toBe(false);
    expect(isBrowserKey(key("ArrowRight", { altKey: true }, { tagName: "TEXTAREA" }))).toBe(false);
    expect(isBrowserKey(key("F5", {}, field))).toBe(true);
    expect(isBrowserKey(key("ArrowLeft", { altKey: true }, { tagName: "INPUT", type: "range" }))).toBe(true);
  });

  it("leaves the show's and the editor's own keys alone", () => {
    for (const press of [key(" "), key("ArrowLeft"), key("1"), key("m"), key("z", { ctrlKey: true }), key("Escape"), key("F2")]) {
      expect(isBrowserKey(press), JSON.stringify(press)).toBe(false);
    }
  });

  it("prevents the browser's menu except in text fields, and drops nowhere", () => {
    const fire = guardedWindow();
    expect(fire("contextmenu", { target: { tagName: "DIV" } })).toBe(true);
    expect(fire("contextmenu", { target: { tagName: "INPUT", type: "text" } })).toBe(false);
    expect(fire("contextmenu", { target: { tagName: "TEXTAREA" } })).toBe(false);
    expect(fire("contextmenu", { target: { tagName: "INPUT", type: "range" } })).toBe(true);
    expect(fire("dragover", { target: { tagName: "DIV" } })).toBe(true);
    expect(fire("drop", { target: { tagName: "DIV" } })).toBe(true);
  });

  it("acts even where a part stops the event, as a popup menu does", () => {
    const fire = guardedWindow();
    const stop = (event: GuardEvent) => event.stopPropagation();
    expect(fire("contextmenu", { target: { tagName: "BUTTON" } }, stop)).toBe(true);
    expect(fire("keydown", { ...key("F5"), target: { tagName: "BUTTON" } }, stop)).toBe(true);
    expect(fire("drop", { target: { tagName: "DIV" } }, stop)).toBe(true);
  });
});
