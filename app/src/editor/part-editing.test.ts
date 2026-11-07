import { describe, expect, it } from "vitest";
import { sameParts } from "./part-editing";

describe("sameParts", () => {
  const part = { fromMs: 1000, toMs: 5000 };
  const replay = { part: { fromMs: 2000, toMs: null }, delayMs: 500 };

  it("holds parts that play the same, whatever objects they are", () => {
    expect(sameParts({ part, replay }, { part: { ...part }, replay: { part: { ...replay.part }, delayMs: 500 } })).toBe(true);
    expect(sameParts({ part, replay: null }, { part, replay: null })).toBe(true);
  });

  it("tells a changed part, replay or delay", () => {
    expect(sameParts({ part, replay: null }, { part: { ...part, toMs: null }, replay: null })).toBe(false);
    expect(sameParts({ part, replay }, { part, replay: null })).toBe(false);
    expect(sameParts({ part, replay }, { part, replay: { ...replay, delayMs: 0 } })).toBe(false);
    expect(sameParts({ part, replay }, { part, replay: { ...replay, part: { fromMs: 2000, toMs: 3000 } } })).toBe(false);
  });
});
