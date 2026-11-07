import { describe, expect, it } from "vitest";
import { ANSWER_LETTERS, BUTTONS, buttonName } from "./answers";
import { MAX_ANSWERS, MAX_EVERYONE_ANSWERS } from "./bindings/Constants";
import { translator } from "./i18n";

describe("answer letters", () => {
  it("name every answer, the first ones after the answer buttons", () => {
    expect(ANSWER_LETTERS).toHaveLength(MAX_ANSWERS);
    const buttons = BUTTONS.filter((b) => b !== "big").map((b) => b.toUpperCase());
    expect(buttons).toHaveLength(MAX_EVERYONE_ANSWERS);
    expect(buttons).toEqual(ANSWER_LETTERS.slice(0, MAX_EVERYONE_ANSWERS));
  });

  it("name the big button in words", () => {
    expect(BUTTONS.map((b) => buttonName(translator("en"), b))).toEqual(["A", "B", "C", "D", "big button"]);
    expect(buttonName(translator("cs"), "big")).toBe("velké tlačítko");
  });
});
