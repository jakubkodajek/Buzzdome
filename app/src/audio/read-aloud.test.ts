import { describe, expect, it } from "vitest";
import { DEFAULT_QUIZ_SETTINGS } from "../bindings/Constants";
import { translator } from "../i18n";
import type { Quiz, QuizSettings, Slide } from "../types";
import { mayTakeVoice, pickSystemVoice, questionLines, sampleUrl, soundsItself, spokenText, VOICES, voiceLine, voiceName, type SystemVoice } from "./read-aloud";

const settings: QuizSettings = {
  ...DEFAULT_QUIZ_SETTINGS,
  readAloud: "questionAndAnswers",
  voice: 3,
};

function question(text: string, answers: string[]): Slide {
  return {
    id: text,
    notes: "",
    picture: null,
    video: null,
    sound: null,
    background: null,
    type: "question",
    text,
    mode: "everyone",
    answers,
    correct: [],
    timeLimitS: 20,
    points: 100,
    pointsEnd: null,
    penalty: 0,
  };
}

describe("spokenText", () => {
  it("reads the question as a sentence, and the answers after their letters", () => {
    const q = {
      text: "Kolik nohou\n má pavouk",
      answers: ["Šest", "", "Osm!"],
    };
    expect(spokenText(q, "question")).toBe("Kolik nohou má pavouk.");
    expect(spokenText(q, "questionAndAnswers")).toBe("Kolik nohou má pavouk. A: Šest. C: Osm!");
    expect(spokenText(q, "off")).toBe("");
    expect(spokenText({ text: "Kdo?", answers: [] }, "questionAndAnswers")).toBe("Kdo?");
  });
});

describe("voiceLine and questionLines", () => {
  it("read questions only, each once, in the quiz's language and voice", () => {
    const quiz = {
      slides: [
        question("Kdo?", ["Já"]),
        {
          ...question("x", []),
          type: "billboard",
          title: "Kolo 1",
          text: "",
        } as Slide,
        question("Kdo?", ["Já"]),
        question("Kde?", []),
      ],
      settings,
    } as unknown as Quiz;
    expect(questionLines(quiz)).toEqual([
      { text: "Kdo? A: Já.", language: "cs", voice: 3 },
      { text: "Kde?", language: "cs", voice: 3 },
    ]);
    expect(voiceLine(question("  ", []), settings)).toBeNull();
    expect(voiceLine(null, settings)).toBeNull();
    expect(voiceLine(question("Kdo?", ["Já"]), settings, "question")?.text).toBe("Kdo?");
  });
});

describe("pickSystemVoice", () => {
  const voice = (name: string, lang: string, localService = true, isDefault = false): SystemVoice => ({ name, lang, localService, default: isDefault });

  it("prefers a voice on this computer, then the language's own region", () => {
    const online = voice("Online", "cs-CZ", false);
    const jakub = voice("Jakub", "cs-CZ");
    const english = voice("Zira", "en-US", true, true);
    expect(pickSystemVoice([online, english, jakub], "cs")).toBe(jakub);
    expect(pickSystemVoice([online, english], "cs")).toBe(online);
    expect(pickSystemVoice([voice("David", "en-US"), voice("Hazel", "en_GB")], "en")?.name).toBe("Hazel");
    expect(pickSystemVoice([english], "cs")).toBeNull();
  });
});

describe("voices", () => {
  it("names each voice and ships a sample of it in every language", () => {
    const t = translator("en");
    expect(VOICES.map((v) => voiceName(t, v))).toEqual([
      "Female voice 1",
      "Female voice 2",
      "Female voice 3",
      "Female voice 4",
      "Female voice 5",
      "Male voice 1",
      "Male voice 2",
      "Male voice 3",
      "Male voice 4",
      "Male voice 5",
    ]);
    for (const language of ["cs", "en"] as const) for (const v of VOICES) expect(sampleUrl(language, v), `${language}-${v}`).not.toBeNull();
  });

  it("lets a try or a sample stop anything but the show's reading", () => {
    expect(mayTakeVoice(null, "sample cs-0")).toBe(true);
    expect(mayTakeVoice("preview", "sample cs-0")).toBe(true);
    expect(mayTakeVoice("sample cs-0", "preview")).toBe(true);
    expect(mayTakeVoice("sample cs-0", "show")).toBe(true);
    expect(mayTakeVoice("show", "show")).toBe(true);
    expect(mayTakeVoice("show", "preview")).toBe(false);
    expect(mayTakeVoice("show", "sample en-3")).toBe(false);
  });
});

describe("soundsItself", () => {
  const sound = (start: "onSlide" | "withClock" | "onDemand", volume = 80) =>
    ({ file: "a.mp3", start, repeat: false, volume, part: { fromMs: 0, toMs: null } }) as unknown as NonNullable<Slide["sound"]>;
  const video = (autoplay: boolean, muted = false) => ({ file: "v.mp4", autoplay, muted, volume: 80, introPauseMs: 0 }) as unknown as NonNullable<Slide["video"]>;
  const slide = (change: Partial<Slide>) => ({ ...question("Co hraje?", []), ...change }) as Slide;

  it("leaves a slide with its own sound, however it starts, to be read when asked", () => {
    expect(soundsItself(slide({}))).toBe(false);
    for (const start of ["onSlide", "withClock", "onDemand"] as const) expect(soundsItself(slide({ sound: sound(start) })), start).toBe(true);
    expect(soundsItself(slide({ sound: sound("withClock", 0) }))).toBe(false);
  });

  it("counts a video only when it plays by itself and is heard", () => {
    expect(soundsItself(slide({ video: video(true) }))).toBe(true);
    expect(soundsItself(slide({ video: video(false) }))).toBe(false);
    expect(soundsItself(slide({ video: video(true, true) }))).toBe(false);
  });
});
