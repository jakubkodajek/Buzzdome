import { describe, expect, it } from "vitest";
import { isColor } from "../theme/color";
import { RECENT_LIMIT, SHADES, STANDARD_COLORS, withRecent } from "./palette";

describe("the colour palette", () => {
  it("has rows of shades from white to black in its first column, all as a quiz file writes colours", () => {
    expect(SHADES.map((row) => row[0])).toEqual(["#FFFFFF", "#D9D9D9", "#999999", "#595959", "#000000"]);
    expect(new Set(SHADES.map((row) => row.length))).toEqual(new Set([10]));
    expect([...SHADES.flat(), ...STANDARD_COLORS].every(isColor)).toBe(true);
    expect(new Set(SHADES.flat()).size).toBe(50);
  });

  it("keeps recent colours the latest first, each once, up to its limit", () => {
    let recent = withRecent([], "#111111");
    recent = withRecent(recent, "#222222");
    recent = withRecent(recent, "#111111");
    expect(recent).toEqual(["#111111", "#222222"]);
    for (let i = 0; i < 20; i++) recent = withRecent(recent, `#0000${String(i).padStart(2, "0")}`);
    expect(recent).toHaveLength(RECENT_LIMIT);
    expect(recent[0]).toBe("#000019");
  });
});
