import { afterEach, describe, expect, it, vi } from "vitest";
import { menuWords, translator, windowLang, type Lang } from "./i18n";

describe("i18n", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("one switch reaches every part of the window, and a switch in the other window too", () => {
    const window = new EventTarget();
    vi.stubGlobal("window", window);
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => storage.set(key, value) });
    const [menu, director] = [vi.fn(), vi.fn()];
    const stops = [windowLang.subscribe(menu), windowLang.subscribe(director)];
    windowLang.set("en");
    expect([windowLang.get(), menu.mock.calls.length, director.mock.calls.length]).toEqual(["en", 1, 1]);
    // The other window switched back: it wrote the setting, and this window hears of it.
    for (const [key] of storage) storage.set(key, "cs");
    window.dispatchEvent(new Event("storage"));
    expect([windowLang.get(), menu.mock.calls.length]).toEqual(["cs", 2]);
    for (const stop of stops) stop();
  });

  it("words the whole macOS menu in both languages, with the app's name where it belongs", () => {
    for (const lang of ["cs", "en"] as Lang[]) {
      const words = menuWords(translator(lang));
      for (const [item, word] of Object.entries(words)) expect(word, `${lang} ${item}`).not.toBe("");
      for (const item of ["quit", "about", "hide"] as const) expect(words[item]).toContain("Buzzdome");
    }
    expect(menuWords(translator("cs")).copy).toBe("Kopírovat");
  });
});
