import { describe, expect, it } from "vitest";
import { canCreate, canSend, entry, turnsOf } from "./ai-chat";

describe("the AI panel's conversation", () => {
  const talk = [
    entry({ role: "user", text: "Kvíz o Praze", files: [] }),
    entry({ role: "assistant", text: "Navrhuji 3 kola.", cutOff: false, sources: [] }),
    entry({ role: "note", text: "Vložené otázky: 15." }),
    entry({ role: "assistant", text: "  ", cutOff: true, sources: [] }),
  ];

  it("gives the model the quizmaster's turns and its own, not the app's notes or empty turns", () => {
    expect(turnsOf(talk)).toEqual([
      { role: "user", text: "Kvíz o Praze" },
      { role: "assistant", text: "Navrhuji 3 kola." },
    ]);
    expect(new Set(talk.map((shown) => shown.id)).size).toBe(talk.length);
  });

  it("sends words, or the files alone to begin with", () => {
    expect(canSend("Ještě kolo o mostech", 0, talk)).toBe(true);
    expect(canSend("  ", 0, [])).toBe(false);
    expect(canSend("", 2, [])).toBe(true);
    expect(canSend("", 2, talk)).toBe(false);
  });

  it("makes the questions from the conversation, words typed or the files alone", () => {
    expect(canCreate("", 0, talk)).toBe(true);
    expect(canCreate("", 0, [])).toBe(false);
    expect(canCreate("", 1, [])).toBe(true);
    expect(canCreate("", 0, [entry({ role: "note", text: "x" })])).toBe(false);
  });
});
