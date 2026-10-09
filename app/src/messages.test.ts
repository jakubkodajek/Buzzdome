import { describe, expect, it } from "vitest";
import { translator } from "./i18n";
import { errorText, problemText, protocolText, receiverProblemText } from "./messages";

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

  it("numbers slides and monitors from one, as the quizmaster sees them", () => {
    expect(errorText(en, { code: "noSuchSlide", slide: 0 })).toBe("There is no slide 1.");
    expect(errorText(en, { code: "noSuchMonitor", index: 1 })).toBe("Display 2 is not connected.");
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

describe("receiver texts", () => {
  it("adds what to check on the platform when the receiver cannot be opened", () => {
    expect(receiverProblemText(en, "cannotOpen", "denied", "linux")).toContain("udev rule");
    expect(receiverProblemText(en, "cannotOpen", "denied", "macos")).toContain("Input Monitoring");
    expect(receiverProblemText(en, "connectionLost", "gone", "linux")).toBe("The receiver connection was lost (gone). Reconnecting…");
  });

  it("names the protocol", () => {
    expect(protocolText(cs, { type: "learned", buttons: 5 })).toBe("naučená tlačítka (5)");
    expect(protocolText(en, { type: "rawOnly" })).toBe("raw reports only");
  });
});
