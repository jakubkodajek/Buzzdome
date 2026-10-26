import { describe, expect, it } from "vitest";
import { DEFAULT_QUIZ_SOUNDS, EFFECT_SLOTS, MUSIC_SLOTS, SOUND_SLOTS } from "../bindings/Constants";
import type { QuizSounds } from "../types";
import { choiceOf, defaultChoice, isMusic, LIBRARY, libraryUrl, sameSounds, SLOT_SOUNDS, SOUND_NAME, soundSource, withChoice } from "./library";
import { roundsDue } from "./show-audio";

const sounds = (slots: QuizSounds["slots"]): QuizSounds => ({ ...DEFAULT_QUIZ_SOUNDS, slots });
const noFiles = () => null;

describe("sound library", () => {
  it("has a file and a name for each sound, and a sound for each name", () => {
    for (const sound of LIBRARY) {
      expect(libraryUrl(sound.id), sound.id).toBeTruthy();
      expect(SOUND_NAME[sound.id], sound.id).toBeTruthy();
    }
    expect(Object.keys(SOUND_NAME).sort()).toEqual(LIBRARY.map((sound) => sound.id).sort());
  });

  it("makes each effect for one moment, each piece of music for one or more, and loops music only", () => {
    const madeFor = (slots: readonly string[], id: string) => slots.filter((slot) => SLOT_SOUNDS[slot as keyof typeof SLOT_SOUNDS].madeFor.includes(id));
    for (const sound of LIBRARY) {
      if (sound.kind === "effect") {
        expect(madeFor(EFFECT_SLOTS, sound.id), sound.id).toHaveLength(1);
        expect(sound.loopMs, sound.id).toBeUndefined();
      } else {
        expect(madeFor(MUSIC_SLOTS, sound.id).length, sound.id).toBeGreaterThan(0);
        expect(sound.loopMs, sound.id).toBeGreaterThan(0);
        expect(sound.loopMs, sound.id).toBeLessThan(sound.ms);
      }
    }
    for (const slot of SOUND_SLOTS) {
      for (const id of SLOT_SOUNDS[slot].madeFor) expect(LIBRARY.find((sound) => sound.id === id)?.kind, id).toBe(isMusic(slot) ? "music" : "effect");
    }
    expect([...EFFECT_SLOTS, ...MUSIC_SLOTS].sort()).toEqual([...SOUND_SLOTS].sort());
  });

  it("plays each effect's first sound by default, and no music", () => {
    expect(defaultChoice("buzz")).toEqual({ builtIn: "buzz.classic" });
    expect(MUSIC_SLOTS.map(defaultChoice)).toEqual(MUSIC_SLOTS.map(() => "none"));
    // Each effect's countdown is as long as the last seconds it plays over.
    expect(SLOT_SOUNDS.countdown.madeFor.map((id) => LIBRARY.find((sound) => sound.id === id)?.ms)).toEqual([5000, 5000, 5000]);
  });

  it("plays the default for a moment left out, and for a sound the app does not have or that does not fit", () => {
    const chosen = sounds({ buzz: { builtIn: "buzz.horn" }, wrong: { builtIn: "buzz.from-a-newer-build" }, correct: { builtIn: "music.victory" }, timeUp: "none", lobbyMusic: { file: "media/intro.mp3" } });
    expect(choiceOf(chosen, "buzz")).toEqual({ builtIn: "buzz.horn" });
    expect(choiceOf(chosen, "wrong")).toEqual(defaultChoice("wrong"));
    expect(choiceOf(chosen, "correct")).toEqual(defaultChoice("correct"));
    expect(choiceOf(chosen, "timeUp")).toBe("none");
    expect(choiceOf(chosen, "lobbyMusic")).toEqual({ file: "media/intro.mp3" });
    expect(choiceOf(chosen, "reveal")).toEqual(defaultChoice("reveal"));
  });

  it("keeps only the choices that are not the default", () => {
    const chosen = withChoice(sounds({}), "buzz", { builtIn: "buzz.bell" });
    expect(chosen.slots).toEqual({ buzz: { builtIn: "buzz.bell" } });
    expect(withChoice(chosen, "buzz", { builtIn: "buzz.classic" }).slots).toEqual({});
    expect(withChoice(chosen, "clockMusic", "none").slots).toEqual({ buzz: { builtIn: "buzz.bell" } });
    expect(sameSounds(withChoice(chosen, "buzz", defaultChoice("buzz")), sounds({}))).toBe(true);
    expect(sameSounds(chosen, { ...chosen, musicVolume: 10 })).toBe(false);
  });

  it("finds the file a choice plays", () => {
    const fileUrl = (reference: string) => `asset://quiz/${reference}`;
    expect(soundSource("buzz", "none", fileUrl)).toBeNull();
    expect(soundSource("lobbyMusic", { file: "media/intro.mp3" }, fileUrl)).toEqual({ url: "asset://quiz/media/intro.mp3", builtIn: false, loopMs: null });
    expect(soundSource("lobbyMusic", { file: "media/intro.mp3" }, noFiles)).toBeNull();
    const lounge = soundSource("betweenMusic", { builtIn: "music.lounge" }, noFiles);
    expect(lounge).toMatchObject({ builtIn: true, loopMs: LIBRARY.find((sound) => sound.id === "music.lounge")?.loopMs });
    expect(soundSource("buzz", { builtIn: "music.lounge" }, noFiles)).toBeNull();
  });

  it("sets each round of a loop to start a loop's length after the one before", () => {
    expect(roundsDue(10, 12, 0.75)).toEqual({ starts: [10, 10.75, 11.5], next: 12.25 });
    expect(roundsDue(12.25, 12, 0.75)).toEqual({ starts: [], next: 12.25 });
  });
});
