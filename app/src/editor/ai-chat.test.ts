import { describe, expect, it } from "vitest";
import { canCreate, canSend, entry, replyBlocks, runsOf, takeBack, turnsOf, unsent } from "./ai-chat";

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

  it("sends words, or new files alone", () => {
    expect(canSend("Ještě kolo o mostech", 0)).toBe(true);
    expect(canSend("  ", 0)).toBe(false);
    expect(canSend("", 2)).toBe(true);
  });

  it("makes the questions from the conversation, words typed or the files alone", () => {
    expect(canCreate("", 0, talk)).toBe(true);
    expect(canCreate("", 0, [])).toBe(false);
    expect(canCreate("", 1, [])).toBe(true);
    expect(canCreate("", 0, [entry({ role: "note", text: "x" })])).toBe(false);
  });

  it("shows in the message box only the files not sent with a message", () => {
    const files = [{ id: 7 }, { id: 8 }, { id: 9 }];
    const sent = [entry({ role: "user", text: "", files: [7] }), entry({ role: "user", text: "A tohle?", files: [9] })];
    expect(unsent(files, sent)).toEqual([{ id: 8 }]);
    expect(turnsOf(sent)).toEqual([
      { role: "user", text: "" },
      { role: "user", text: "A tohle?" },
    ]);
  });

  it("takes back a message the model did not answer, and only that", () => {
    const asked = entry({ role: "user", text: "A co historie?", files: [] });
    const back = takeBack([...talk, asked], asked.id, "");
    expect(back).toEqual({ entries: talk, message: "A co historie?" });
    // Words typed meanwhile stay in the box.
    expect(takeBack([...talk, asked], asked.id, "Jiný nápad")?.message).toBe("Jiný nápad");
    // Answered: nothing is taken back.
    const answer = entry({ role: "assistant", text: "Tady je.", cutOff: false, sources: [] });
    expect(takeBack([...talk, asked, answer], asked.id, "")).toBeNull();
  });
});

describe("the model's reply", () => {
  it("reads paragraphs, headings, lists and bold text", () => {
    const blocks = replyBlocks("Navrhuji **3 kola**:\n\n## Kola\n1. Památky\n2. Historie\n   od Karla IV.\n- lehké\n- těžké\n\nMají být lehké?");
    expect(blocks).toEqual([
      { kind: "paragraph", lines: [[{ text: "Navrhuji ", bold: false }, { text: "3 kola", bold: true }, { text: ":", bold: false }]] },
      { kind: "heading", runs: [{ text: "Kola", bold: false }] },
      { kind: "list", ordered: true, start: 1, items: [[{ text: "Památky", bold: false }], [{ text: "Historie", bold: false }, { text: " od Karla IV.", bold: false }]] },
      { kind: "list", ordered: false, start: 1, items: [[{ text: "lehké", bold: false }], [{ text: "těžké", bold: false }]] },
      { kind: "paragraph", lines: [[{ text: "Mají být lehké?", bold: false }]] },
    ]);
  });

  it("keeps a lone star and plain lines as they are", () => {
    expect(runsOf("5 * 3 = **15")).toEqual([{ text: "5 * 3 = **15", bold: false }]);
    expect(replyBlocks("a\nb")).toEqual([{ kind: "paragraph", lines: [[{ text: "a", bold: false }], [{ text: "b", bold: false }]] }]);
  });
});
