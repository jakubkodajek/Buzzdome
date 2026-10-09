import { describe, expect, it } from "vitest";
import { ANSWER_LETTERS, BUTTONS } from "./answers";
import { MAX_ANSWERS } from "./bindings/Constants";

describe("answer letters", () => {
  it("name one answer per answer button", () => {
    expect(ANSWER_LETTERS).toHaveLength(MAX_ANSWERS);
    expect(BUTTONS.filter((b) => b !== "big").map((b) => b.toUpperCase())).toEqual([...ANSWER_LETTERS]);
  });
});
