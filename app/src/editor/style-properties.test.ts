import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { Slide } from "../types";
import { newSlide, setTimer } from "./slides";
import { styleProperties } from "./style-properties";

const t = translator("en");

describe("a question's clock design", () => {
  it("is chosen from every design through its row, the default stored as none", () => {
    let slide: Slide = newSlide("question", "q");
    const choice = () => {
      const value = styleProperties(t, { slide, update: (change) => (slide = change(slide)) }).timer.value;
      if (value?.kind !== "choice") throw new Error("the clock design is a choice");
      return value;
    };
    expect(choice()).toMatchObject({ value: "ring", initial: "ring", options: ["ring", "bar", "digital", "hourglass", "stopwatch"] });
    choice().set("digital");
    expect(slide.style).toEqual({ timer: "digital" });
    expect(choice().value).toBe("digital");
    choice().set("ring");
    expect(slide.style).toBeUndefined();
    expect(styleProperties(t, { slide: setTimer(slide, "bar"), update: () => {} }).timer.value?.value).toBe("bar");
  });
});
