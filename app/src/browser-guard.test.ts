import { describe, expect, it } from "vitest";
import { installBrowserGuard, isBrowserKey } from "./browser-guard";

const key = (key: string, mods: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

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

  it("leaves the show's and the editor's own keys alone", () => {
    for (const press of [key(" "), key("ArrowLeft"), key("1"), key("m"), key("z", { ctrlKey: true }), key("Escape"), key("F2")]) {
      expect(isBrowserKey(press), JSON.stringify(press)).toBe(false);
    }
  });

  it("prevents the browser's menu except in text fields, and drops nowhere", () => {
    const handlers = new Map<string, (event: { target: unknown; key?: string; preventDefault: () => void }) => void>();
    installBrowserGuard({ addEventListener: ((type: string, handler: never) => handlers.set(type, handler)) as never });
    const fire = (type: string, target: unknown) => {
      let prevented = false;
      handlers.get(type)?.({ target, preventDefault: () => (prevented = true) });
      return prevented;
    };
    expect(fire("contextmenu", { tagName: "DIV" })).toBe(true);
    expect(fire("contextmenu", { tagName: "INPUT", type: "text" })).toBe(false);
    expect(fire("contextmenu", { tagName: "TEXTAREA" })).toBe(false);
    expect(fire("contextmenu", { tagName: "INPUT", type: "range" })).toBe(true);
    expect(fire("dragover", { tagName: "DIV" })).toBe(true);
    expect(fire("drop", { tagName: "DIV" })).toBe(true);
  });
});
