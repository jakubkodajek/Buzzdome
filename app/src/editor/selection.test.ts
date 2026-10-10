import { describe, expect, it } from "vitest";
import type { Quiz, Slide } from "../types";
import { applyEdit, type Edit } from "./history";
import { clickSlide, FIRST, insertSlides, moveByOne, moveToGap, positions, removeSlides, selectAll, selectedIds, selectOne, type Selection } from "./selection";

const slide = (id: string): Slide => ({ id, type: "billboard", title: id, text: "", notes: "", picture: null, video: null, sound: null, background: null });
const slides = ["a", "b", "c", "d", "e"].map(slide);
const quiz: Quiz = { schemaVersion: 4, title: "", theme: null, slides } as unknown as Quiz;

const order = (edit: Edit | null) => (edit ? applyEdit(quiz, edit) : quiz).slides.map((s) => s.id).join("");
const plain = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };
const shift = { ctrl: false, shift: true };

/** The selected ids, the one on the canvas marked with *. */
function shown(selection: Selection): string {
  return selectedIds(selection, slides)
    .map((id) => (id === selection.id ? `${id}*` : id))
    .join(" ");
}

describe("clicking slides", () => {
  it("selects one slide, or adds and removes slides with Ctrl", () => {
    let selection = clickSlide(FIRST, slides, 1, plain);
    expect(shown(selection)).toBe("b*");
    selection = clickSlide(selection, slides, 3, ctrl);
    expect(shown(selection)).toBe("b d*");
    selection = clickSlide(selection, slides, 0, ctrl);
    expect(shown(selection)).toBe("a* b d");
    selection = clickSlide(selection, slides, 1, ctrl);
    expect(shown(selection)).toBe("a* d");
    // Taking out the slide on the canvas shows the last one selected before it.
    selection = clickSlide(selection, slides, 0, ctrl);
    expect(shown(selection)).toBe("d*");
    expect(selection.index).toBe(3);
    // The last selected slide stays.
    expect(shown(clickSlide(selection, slides, 3, ctrl))).toBe("d*");
    expect(shown(clickSlide(selection, slides, 4, plain))).toBe("e*");
  });

  it("selects a range from the last plain or Ctrl click with Shift", () => {
    let selection = clickSlide(FIRST, slides, 1, plain);
    selection = clickSlide(selection, slides, 3, shift);
    expect(shown(selection)).toBe("b c d*");
    // The range starts at the same slide again.
    expect(shown(clickSlide(selection, slides, 0, shift))).toBe("a* b");
    selection = clickSlide(selectOne(slides, 0), slides, 4, ctrl);
    expect(shown(clickSlide(selection, slides, 2, { ctrl: true, shift: true }))).toBe("a c* d e");
  });

  it("selects every slide, keeping the one on the canvas", () => {
    expect(shown(selectAll(selectOne(slides, 2), slides, 2))).toBe("a b c* d e");
  });

  it("forgets slides that are gone", () => {
    const selection: Selection = { ...selectOne(slides, 0), others: ["zz", "c"] };
    expect(selectedIds(selection, slides)).toEqual(["a", "c"]);
    expect(positions(slides, ["d", "a"])).toEqual([0, 3]);
  });
});

describe("changing several slides at once", () => {
  it("removes and inserts them as one change", () => {
    expect(order(removeSlides([3, 1]))).toBe("ace");
    expect(order(insertSlides([slide("x"), slide("y")], 2))).toBe("abxycde");
  });

  it("moves them together into a gap, in their order", () => {
    expect(order(moveToGap(slides, ["b", "d"], 0))).toBe("bdace");
    expect(order(moveToGap(slides, ["a", "c"], 5))).toBe("bdeac");
    expect(order(moveToGap(slides, ["a", "e"], 3))).toBe("bcaed");
    expect(moveToGap(slides, ["b", "c"], 1)).toBeNull();
    expect(moveToGap(slides, ["b", "c"], 3)).toBeNull();
  });

  it("moves them one place up or down each, unless one is at the end", () => {
    expect(order(moveByOne(slides, ["b", "d"], -1))).toBe("badce");
    expect(order(moveByOne(slides, ["b", "c"], 1))).toBe("adbce");
    expect(moveByOne(slides, ["a", "c"], -1)).toBeNull();
    expect(moveByOne(slides, ["e"], 1)).toBeNull();
  });
});
