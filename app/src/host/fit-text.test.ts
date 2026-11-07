import { describe, expect, it } from "vitest";
import { FIT_FLOOR, fitScale } from "./fit-text";

describe("fitScale", () => {
  it("keeps text that fits at its size", () => {
    expect(fitScale(() => false)).toBe(1);
  });

  it("draws text that does not fit as large as it fits", () => {
    expect(fitScale((scale) => scale > 0.8)).toBe(0.8);
    expect(fitScale((scale) => scale > 0.75)).toBe(0.75);
  });

  it("stops at the floor when nothing fits", () => {
    const tried: number[] = [];
    expect(
      fitScale((scale) => {
        tried.push(scale);
        return true;
      }),
    ).toBe(FIT_FLOOR);
    expect(Math.min(...tried)).toBeGreaterThan(FIT_FLOOR);
  });
});
