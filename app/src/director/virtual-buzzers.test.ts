import { describe, expect, it } from "vitest";
import { buzzerCount } from "./virtual-buzzers";

describe("buzzerCount", () => {
  it("reads a typed count within the bounds", () => {
    expect(buzzerCount("8")).toBe(8);
    expect(buzzerCount("0")).toBe(1);
    expect(buzzerCount("250")).toBe(100);
    expect(buzzerCount("7.6")).toBe(8);
  });

  it("keeps the fallback for no number, so clearing the field to type another is no count", () => {
    expect(buzzerCount("", 12)).toBe(12);
    expect(buzzerCount("  ", 12)).toBe(12);
    expect(buzzerCount("abc", 12)).toBe(12);
    expect(buzzerCount(null)).toBe(6);
  });
});
