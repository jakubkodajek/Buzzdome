import { describe, expect, it } from "vitest";
import { parseTeamNames } from "./team-names";

describe("parseTeamNames", () => {
  it("takes one name per line and skips blank lines", () => {
    expect(parseTeamNames("\uFEFFSokoli\r\n\r\n  Pivní rytíři \nKvízoví mágové, s.r.o.\rStůl 4\n")).toEqual(["Sokoli", "Pivní rytíři", "Kvízoví mágové, s.r.o.", "Stůl 4"]);
  });
});
