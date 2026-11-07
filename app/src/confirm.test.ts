import { describe, expect, it } from "vitest";
import { saveChoice } from "./confirm";

describe("saveChoice", () => {
  const labels = { yes: "Uložit", no: "Neukládat" };

  it("reads the button pressed from its label", () => {
    expect(saveChoice("Uložit", labels)).toBe("save");
    expect(saveChoice("Neukládat", labels)).toBe("discard");
    expect(saveChoice("Zrušit", labels)).toBe("cancel");
  });

  it("cancels on any other answer, so nothing is lost unasked", () => {
    for (const answer of ["Cancel", "Yes", "No", "Ok", ""]) expect(saveChoice(answer, labels)).toBe("cancel");
  });
});
