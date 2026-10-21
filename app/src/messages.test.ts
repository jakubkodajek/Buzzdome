import { describe, expect, it } from "vitest";
import { translator } from "./i18n";
import { awaitsAnswer, errorText, importIssueText, placesText, problemText, protocolText, receiverProblemText, settingsLostText, thrownText } from "./messages";

const cs = translator("cs");
const en = translator("en");

describe("errorText", () => {
  it("words backend codes with their parameters", () => {
    expect(errorText(cs, { code: "notNow" })).toBe("V tomto kroku to nejde.");
    expect(errorText(en, { code: "unknownBuzzer", buzzer: 7 })).toBe("Buzzer 7 is not in the show.");
    expect(errorText(en, { code: "tooLarge", maxMb: 4096 })).toBe("The quiz is larger than 4096 MB unpacked.");
  });

  it("words each file action in its own sentence", () => {
    const error = { code: "file", path: "/q/a.quiz", detail: "denied" } as const;
    expect(errorText(cs, { ...error, action: "read" })).toBe("Soubor /q/a.quiz nejde přečíst: denied");
    expect(errorText(en, { ...error, action: "createFolder" })).toBe("Cannot create the folder /q/a.quiz: denied");
  });

  it("names the button that resumes a show as the Director labels it", () => {
    expect(errorText(cs, { code: "journalBroken" })).toContain("tlačítkem Pokračovat v poslední show;");
    expect(errorText(en, { code: "journalBroken" })).toContain("with Resume last show;");
  });

  it("numbers slides from one, as the quizmaster sees them", () => {
    expect(errorText(en, { code: "noSuchSlide", slide: 0 })).toBe("There is no slide 1.");
  });

  it("names the button of a learn conflict and the first problem of an invalid quiz", () => {
    expect(errorText(en, { code: "learnConflict", buzzer: 2, button: "big" })).toContain("buzzer 2, button ●");
    const problems = [{ slideId: "q1", problem: { code: "noCorrectAnswer" } }] as const;
    expect(errorText(en, { code: "quizInvalid", problems: [...problems] })).toBe(
      "The quiz cannot be played; problems: 1. An everyone-answers question needs a correct answer.",
    );
  });

  it("shows anything else as it is", () => {
    expect(errorText(cs, "command not found")).toBe("command not found");
    expect(errorText(cs, new Error("boom"))).toBe("boom");
  });
});

describe("thrownText", () => {
  it("gives anything thrown as text, and never throws itself", () => {
    expect(thrownText(new Error("boom"))).toBe("boom");
    expect(thrownText("command not found")).toBe("command not found");
    expect(thrownText({ code: "notNow" })).toBe('{"code":"notNow"}');
    expect(thrownText(undefined)).toBe("undefined");
    const itself: { me?: unknown } = {};
    itself.me = itself;
    expect(thrownText(itself)).toBe("[object Object]");
    expect(thrownText(10n)).toBe("10");
  });
});

describe("problemText", () => {
  it("fills every occurrence of a parameter", () => {
    expect(problemText(en, { code: "pointsOutOfRange", max: 5 })).toBe("Points must be between -5 and 5.");
  });

  it("says which media types a file may be", () => {
    const issue = { type: "wrongType", expected: ["image", "video"] } as const;
    expect(problemText(cs, { code: "media", file: "media/a.txt", issue: { ...issue, expected: [...issue.expected] } })).toBe(
      "media/a.txt: musí to být obrázek (png, jpg, jpeg, webp, gif, svg) nebo video (mp4, webm)",
    );
    expect(problemText(en, { code: "media", file: "media/a.png", issue: { type: "missing" } })).toBe("media/a.png: the file is missing");
  });
});

describe("import notes", () => {
  it("word each issue with its parameters and leave the format's own braces", () => {
    expect(importIssueText(en, { code: "badSetting", column: "VIDEO", setting: "speed", value: "500" })).toBe(
      "Setting speed=500 in column VIDEO is not valid; the default was kept.",
    );
    expect(importIssueText(cs, { code: "linkedFile" })).toContain("označené {L}");
    expect(importIssueText(en, { code: "notEveryone", answers: 5 })).toContain("with 5 answers");
  });

  it("list the rows by sheet, a few of them", () => {
    expect(placesText(en, [{ sheet: "", row: 4 }])).toBe("row 4");
    const rows = [3, 5, 7].map((row) => ({ sheet: "Round 1", row }));
    expect(placesText(cs, [...rows, { sheet: "Round 2", row: 2 }])).toBe("list „Round 1“, řádky 3, 5, 7; list „Round 2“, řádek 2");
    const many = Array.from({ length: 11 }, (_, i) => ({ sheet: "", row: i + 2 }));
    expect(placesText(en, many)).toBe("rows 2, 3, 4, 5, 6, 7, 8, 9 and 3 more");
  });
});

describe("receiver texts", () => {
  it("adds what to check on the platform when the receiver cannot be opened", () => {
    expect(receiverProblemText(en, "cannotOpen", "denied", "linux")).toContain("udev rule");
    expect(receiverProblemText(en, "cannotOpen", "denied", "macos")).toContain("Input Monitoring");
    expect(receiverProblemText(en, "connectionLost", "gone", "linux")).toBe("The receiver connection was lost (gone). Reconnecting…");
  });

  it("names the protocol", () => {
    expect(protocolText(cs, { type: "learned", buttons: 5 })).toBe("naučená tlačítka (5)");
    expect(protocolText(en, { type: "rawOnly" })).toBe("raw reports only");
    expect(protocolText(en, { type: "enjoy303s", answered: true })).toBe("Enjoy 303S");
    expect(protocolText(cs, { type: "enjoy303s", answered: false })).toContain("neodpovídá");
    expect(awaitsAnswer({ type: "enjoy303s", answered: false })).toBe(true);
    expect(awaitsAnswer({ type: "enjoy303s", answered: true })).toBe(false);
    expect(awaitsAnswer({ type: "rawOnly" })).toBe(false);
  });

  it("says where a file that was not read went, or that it was left alone", () => {
    expect(settingsLostText(en, { detail: "bad json", keptAs: "/d/receiver.json.broken" })).toBe("bad json The old file was kept as /d/receiver.json.broken.");
    expect(settingsLostText(en, { detail: "in use", keptAs: null })).toBe(
      "in use The file was left as it is and is not written over until the app starts again.",
    );
  });
});
