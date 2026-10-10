import { describe, expect, it } from "vitest";
import { decodeText, parseTeamNames } from "./team-names";

describe("parseTeamNames", () => {
  it("takes one name per line and skips blank lines", () => {
    expect(parseTeamNames("\uFEFFSokoli\r\n\r\n  Pivní rytíři \nKvízoví mágové, s.r.o.\n", false)).toEqual([
      "Sokoli",
      "Pivní rytíři",
      "Kvízoví mágové, s.r.o.",
    ]);
  });

  it("takes the first column of a CSV file, whichever separator it uses", () => {
    expect(parseTeamNames("Sokoli;Stůl 1\nRytíři;Stůl 2", true)).toEqual(["Sokoli", "Rytíři"]);
    expect(parseTeamNames("Sokoli,1\nRytíři,2\n", true)).toEqual(["Sokoli", "Rytíři"]);
    expect(parseTeamNames('"Pivo, víno a my";3\n"Řekli ""ano""";4\n;5', true)).toEqual(["Pivo, víno a my", 'Řekli "ano"']);
  });
});

describe("decodeText", () => {
  it("reads UTF-8, and Windows-1250 when the bytes are not UTF-8", () => {
    expect(decodeText(new TextEncoder().encode("Žluťoučký kůň").buffer)).toBe("Žluťoučký kůň");
    // "Žluťák" in Windows-1250.
    const cp1250 = new Uint8Array([0x8e, 0x6c, 0x75, 0x9d, 0xe1, 0x6b]);
    expect(decodeText(cp1250.buffer)).toBe("Žluťák");
  });
});
