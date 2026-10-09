import { describe, expect, it } from "vitest";
import { LAYOUT_GROUPS, slideFromLayout } from "./layouts";
import { MAX_ANSWERS, MIN_EVERYONE_ANSWERS } from "./slides";

describe("layouts", () => {
  it("only makes questions the show can play", () => {
    for (const layout of LAYOUT_GROUPS.flatMap((group) => group.layouts)) {
      const slide = slideFromLayout(layout, "x");
      if (slide.type !== "question") continue;
      expect(slide.answers.length).toBeLessThanOrEqual(MAX_ANSWERS);
      if (slide.mode === "everyone") {
        expect(slide.answers.length).toBeGreaterThanOrEqual(MIN_EVERYONE_ANSWERS);
        expect(slide.correct).toBe(0);
      } else {
        expect(slide.answers).toEqual([]);
        expect(slide.correct).toBeNull();
      }
    }
  });
});
