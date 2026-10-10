import { afterEach, describe, expect, it, vi } from "vitest";
import { readPreference, writePreference } from "./preferences";

describe("preferences", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("reads back what was written", () => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (key: string) => store.get(key) ?? null, setItem: (key: string, value: string) => store.set(key, value) });
    expect(readPreference("quiz-show.test")).toBeNull();
    writePreference("quiz-show.test", "on");
    expect(readPreference("quiz-show.test")).toBe("on");
  });

  it("never fails without storage", () => {
    const refuse = () => {
      throw new Error("storage is off");
    };
    vi.stubGlobal("localStorage", { getItem: refuse, setItem: refuse });
    expect(readPreference("quiz-show.test")).toBeNull();
    expect(() => writePreference("quiz-show.test", "on")).not.toThrow();
  });
});
