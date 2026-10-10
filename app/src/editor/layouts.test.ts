import { describe, expect, it } from "vitest";
import { LAYOUT_GROUPS, slideFromLayout } from "./layouts";
import { MAX_ANSWERS, MAX_EVERYONE_ANSWERS, MIN_EVERYONE_ANSWERS } from "../bindings/Constants";

describe("layouts", () => {
  it("only makes questions the show can play", () => {
    for (const layout of LAYOUT_GROUPS.flatMap((group) => group.layouts)) {
      const slide = slideFromLayout(layout, "x");
      if (slide.type !== "question") continue;
      expect(slide.answers.length).toBeLessThanOrEqual(MAX_ANSWERS);
      if (slide.mode === "everyone") {
        expect(slide.answers.length).toBeGreaterThanOrEqual(MIN_EVERYONE_ANSWERS);
        expect(slide.answers.length).toBeLessThanOrEqual(MAX_EVERYONE_ANSWERS);
        expect(slide.correct).toBe(0);
      } else {
        expect(slide.correct).toBe(slide.answers.length === 0 ? null : 0);
      }
    }
  });

  it("makes questions with more answers than buttons fastest finger", () => {
    const six = slideFromLayout({ type: "question", answers: MAX_ANSWERS }, "x");
    expect(six.type === "question" && six.mode).toBe("fastestFinger");
  });
});
