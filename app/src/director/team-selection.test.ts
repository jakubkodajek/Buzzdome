import { describe, expect, it } from "vitest";
import { clickSelection, typedBuzzer, typedDelta } from "./team-selection";

const rows = [1, 2, 3, 4, 5];
const plain = { toggle: false, range: false };
const ctrl = { toggle: true, range: false };
const shift = { toggle: false, range: true };
const sorted = (set: Set<number>) => [...set].sort((a, b) => a - b);

describe("clickSelection", () => {
  it("selects a row alone with a plain click, and a second click on it selects nothing", () => {
    expect(sorted(clickSelection(new Set(), rows, 2, plain, null))).toEqual([2]);
    expect(sorted(clickSelection(new Set([1, 3]), rows, 2, plain, 1))).toEqual([2]);
    expect(sorted(clickSelection(new Set([2]), rows, 2, plain, 2))).toEqual([]);
    // One of several: that one alone.
    expect(sorted(clickSelection(new Set([2, 3]), rows, 2, plain, 2))).toEqual([2]);
  });

  it("adds or takes away a row with Ctrl", () => {
    expect(sorted(clickSelection(new Set([1]), rows, 3, ctrl, 1))).toEqual([1, 3]);
    expect(sorted(clickSelection(new Set([1, 3]), rows, 3, ctrl, 1))).toEqual([1]);
  });

  it("selects the rows from the anchor with Shift, either way", () => {
    expect(sorted(clickSelection(new Set([2]), rows, 4, shift, 2))).toEqual([2, 3, 4]);
    expect(sorted(clickSelection(new Set([4]), rows, 2, shift, 4))).toEqual([2, 3, 4]);
    expect(sorted(clickSelection(new Set([5]), rows, 2, { toggle: true, range: true }, 3))).toEqual([2, 3, 5]);
    // No anchor, or one that left the table: a plain click.
    expect(sorted(clickSelection(new Set(), rows, 2, shift, null))).toEqual([2]);
    expect(sorted(clickSelection(new Set(), rows, 2, shift, 9))).toEqual([2]);
  });
});

describe("typed numbers", () => {
  it("takes whole points other than 0, negative ones too", () => {
    expect(["15", "-150", " 7 "].map(typedDelta)).toEqual([15, -150, 7]);
    expect(["", "0", "1.5", "abc"].map(typedDelta)).toEqual([null, null, null, null]);
  });

  it("takes a buzzer number from 1", () => {
    expect(["42", "1"].map(typedBuzzer)).toEqual([42, 1]);
    expect(["", "0", "-3", "2.5"].map(typedBuzzer)).toEqual([null, null, null, null]);
  });
});
