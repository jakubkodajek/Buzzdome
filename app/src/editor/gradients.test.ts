import { describe, expect, it } from "vitest";
import { GRADIENT_PRESETS, isGradient, MY_GRADIENTS_LIMIT, readGradient, withMine } from "./gradients";

describe("gradients", () => {
  it("presets are valid gradients, each once", () => {
    expect(GRADIENT_PRESETS.every(isGradient)).toBe(true);
    expect(new Set(GRADIENT_PRESETS.map((g) => JSON.stringify(g))).size).toBe(GRADIENT_PRESETS.length);
  });

  it("reads a gradient written as JSON and nothing else", () => {
    expect(readGradient('{"from":"#000000","to":"#FFFFFF80","kind":"radial","extra":1}')).toEqual({ from: "#000000", to: "#FFFFFF80", kind: "radial" });
    expect(readGradient('{"from":"black","to":"#FFFFFF","kind":"radial"}')).toBeNull();
    expect(readGradient('{"from":"#000000","to":"#FFFFFF","kind":"conic"}')).toBeNull();
    expect(readGradient("#000000")).toBeNull();
  });

  it("keeps the user's own the latest first, each once, up to five", () => {
    const gradient = (n: number) => ({ from: `#00000${n}`, to: "#FFFFFF", kind: "vertical" as const });
    let mine = withMine([], gradient(1));
    mine = withMine(mine, gradient(2));
    mine = withMine(mine, { ...gradient(1) });
    expect(mine).toEqual([gradient(1), gradient(2)]);
    for (let n = 3; n <= 8; n++) mine = withMine(mine, gradient(n));
    expect(mine).toHaveLength(MY_GRADIENTS_LIMIT);
    expect(mine[0]).toEqual(gradient(8));
  });
});
