import { describe, expect, it } from "vitest";
import { LOOKS, readLook, showsDark } from "./look";

describe("the Director's look", () => {
  it("reads every look it stores, and light for anything else", () => {
    for (const look of LOOKS) expect(readLook(look)).toBe(look);
    for (const stored of [null, "", "Dark", "black"]) expect(readLook(stored)).toBe("light");
  });

  it("is dark when chosen, or when the system is and the system's is chosen", () => {
    expect(showsDark("dark", false)).toBe(true);
    expect(showsDark("light", true)).toBe(false);
    expect(showsDark("system", true)).toBe(true);
    expect(showsDark("system", false)).toBe(false);
  });
});
