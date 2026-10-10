import { describe, expect, it } from "vitest";
import { cssColor, formatColor, fromHsv, parseColor, readColor, toHsv } from "./color";

describe("colours", () => {
  it("reads and writes the quiz file's hex colours", () => {
    expect(parseColor("#1E4A6B")).toEqual({ r: 0x1e, g: 0x4a, b: 0x6b, a: 1 });
    expect(parseColor("#1e4a6b80")).toEqual({ r: 0x1e, g: 0x4a, b: 0x6b, a: 0x80 / 255 });
    expect(parseColor("#fff")).toBeNull();
    expect(parseColor("1E4A6B")).toBeNull();
    expect(formatColor({ r: 30, g: 74, b: 107, a: 1 })).toBe("#1E4A6B");
    // Opacity only when the colour is not solid.
    expect(formatColor({ r: 30, g: 74, b: 107, a: 0.5 })).toBe("#1E4A6B80");
    expect(formatColor({ r: 300, g: -5, b: 0.4, a: 0 })).toBe("#FF000000");
  });

  it("takes a colour typed or pasted from another program", () => {
    expect(readColor("#1e4a6b")).toBe("#1E4A6B");
    expect(readColor("  1E4A6B ")).toBe("#1E4A6B");
    expect(readColor("#abc")).toBe("#AABBCC");
    expect(readColor("abcd")).toBe("#AABBCCDD");
    expect(readColor("#1E4A6BFF")).toBe("#1E4A6B");
    for (const wrong of ["", "#", "#12345", "red", "#1E4A6G", "rgb(1,2,3)"]) expect(readColor(wrong)).toBeNull();
  });

  it("lets a transparency show through on top of the colour's own opacity", () => {
    expect(cssColor("#FF8000")).toBe("rgba(255, 128, 0, 1)");
    expect(cssColor("#FF8000", 25)).toBe("rgba(255, 128, 0, 0.75)");
    expect(cssColor("#FF800080", 50)).toBe("rgba(255, 128, 0, 0.251)");
    expect(cssColor("nonsense")).toBe("transparent");
  });

  it("goes to hue, saturation and value and back", () => {
    expect(toHsv({ r: 255, g: 0, b: 0 })).toEqual({ h: 0, s: 1, v: 1 });
    expect(toHsv({ r: 0, g: 0, b: 0 })).toEqual({ h: 0, s: 0, v: 0 });
    expect(toHsv({ r: 0, g: 0, b: 255 }).h).toBe(240);
    for (const color of ["#1E4A6B", "#FFCC00", "#7F7F7F", "#00FF80", "#C0306A"]) {
      const rgba = parseColor(color)!;
      expect(formatColor(fromHsv(toHsv(rgba), rgba.a))).toBe(color);
    }
  });
});
