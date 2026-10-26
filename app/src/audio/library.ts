// The show's sounds (spec SN-1 to SN-4): what each moment of the show plays,
// from the app's own library (made by `library/make_sounds.py`), from a
// sound file of the quiz, or nothing.

import { LAST_SECONDS_MS, SOUND_SLOTS } from "../bindings/Constants";
import type { StringKey, Translate } from "../i18n";
import { CUE_LABEL } from "../phase";
import type { QuizSounds, SoundChoice, SoundSlot } from "../types";
import manifest from "./library/sounds.json";

/** One of the app's own sounds, as `sounds.json` lists it. */
export interface LibrarySound {
  id: string;
  kind: "effect" | "music";
  /** Length of the file. */
  ms: number;
  /**
   * Music: where the next round starts, so it loops without a seam; the
   * rest of the file rings on under the next round.
   */
  loopMs?: number;
}

export const LIBRARY: readonly LibrarySound[] = manifest as LibrarySound[];

const BY_ID = new Map(LIBRARY.map((sound) => [sound.id, sound]));

/** The files of the library, bundled with the app. */
const URLS = import.meta.glob<string>("./library/*.mp3", { eager: true, query: "?url", import: "default" });

export function libraryUrl(id: string): string | undefined {
  return URLS[`./library/${id}.mp3`];
}

interface SlotSounds {
  label: StringKey;
  music: boolean;
  /**
   * The app's sounds made for the moment. An effect's first one is its
   * default; music plays none unless the quiz says.
   */
  madeFor: readonly string[];
}

/** Each moment of the show: its name and the app's sounds made for it. */
export const SLOT_SOUNDS: Record<SoundSlot, SlotSounds> = {
  clockStart: { label: CUE_LABEL.clockStart, music: false, madeFor: ["start.chime", "start.gong", "start.go"] },
  countdown: { label: "soundCountdown", music: false, madeFor: ["countdown.ticks", "countdown.beeps", "countdown.pulse"] },
  buzz: { label: CUE_LABEL.buzz, music: false, madeFor: ["buzz.classic", "buzz.bell", "buzz.laser", "buzz.horn"] },
  correct: { label: CUE_LABEL.correct, music: false, madeFor: ["correct.chime", "correct.dings", "correct.fanfare"] },
  wrong: { label: CUE_LABEL.wrong, music: false, madeFor: ["wrong.buzzer", "wrong.trombone", "wrong.bonk"] },
  timeUp: { label: CUE_LABEL.timeUp, music: false, madeFor: ["timeup.horn", "timeup.gong", "timeup.alarm"] },
  reveal: { label: CUE_LABEL.reveal, music: false, madeFor: ["reveal.whoosh", "reveal.sparkle", "reveal.sting"] },
  leaderboard: { label: CUE_LABEL.leaderboard, music: false, madeFor: ["leaderboard.rise", "leaderboard.drums", "leaderboard.chord"] },
  place: { label: CUE_LABEL.place, music: false, madeFor: ["place.cymbal", "place.stab", "place.boom"] },
  winner: { label: CUE_LABEL.winner, music: false, madeFor: ["winner.fanfare", "winner.applause", "winner.party"] },
  lobbyMusic: { label: "soundLobbyMusic", music: true, madeFor: ["music.lobby", "music.lounge"] },
  betweenMusic: { label: "soundBetweenMusic", music: true, madeFor: ["music.lounge", "music.lobby"] },
  clockMusic: { label: "soundClockMusic", music: true, madeFor: ["music.thinking", "music.race"] },
  buildUpMusic: { label: "soundBuildUpMusic", music: true, madeFor: ["music.drumroll"] },
  winnerMusic: { label: "soundWinnerMusic", music: true, madeFor: ["music.victory"] },
};

/** What `slot` is called. */
export function slotLabel(t: Translate, slot: SoundSlot): string {
  return t(SLOT_SOUNDS[slot].label, { s: LAST_SECONDS_MS / 1000 });
}

export const EFFECT_SLOTS: readonly SoundSlot[] = SOUND_SLOTS.filter((slot) => !SLOT_SOUNDS[slot].music);
export const MUSIC_SLOTS: readonly SoundSlot[] = SOUND_SLOTS.filter((slot) => SLOT_SOUNDS[slot].music);

/** The name of each of the app's sounds. */
export const SOUND_NAME: Record<string, StringKey> = {
  "start.chime": "soundChime",
  "start.gong": "soundGong",
  "start.go": "soundGo",
  "countdown.ticks": "soundTicks",
  "countdown.beeps": "soundBeeps",
  "countdown.pulse": "soundPulse",
  "buzz.classic": "soundClassicBuzzer",
  "buzz.bell": "soundBell",
  "buzz.laser": "soundLaser",
  "buzz.horn": "soundHorn",
  "correct.chime": "soundChime",
  "correct.dings": "soundDings",
  "correct.fanfare": "soundFanfare",
  "wrong.buzzer": "soundBuzzer",
  "wrong.trombone": "soundSadTrombone",
  "wrong.bonk": "soundBonk",
  "timeup.horn": "soundHorn",
  "timeup.gong": "soundGong",
  "timeup.alarm": "soundAlarm",
  "reveal.whoosh": "soundWhoosh",
  "reveal.sparkle": "soundSparkle",
  "reveal.sting": "soundSting",
  "leaderboard.rise": "soundRise",
  "leaderboard.drums": "soundDrums",
  "leaderboard.chord": "soundChord",
  "place.cymbal": "soundCymbal",
  "place.stab": "soundStab",
  "place.boom": "soundBoom",
  "winner.fanfare": "soundFanfare",
  "winner.applause": "soundApplause",
  "winner.party": "soundParty",
  "music.lobby": "soundWelcome",
  "music.lounge": "soundLounge",
  "music.thinking": "soundThinking",
  "music.race": "soundRace",
  "music.drumroll": "soundDrumRoll",
  "music.victory": "soundVictory",
};

/** What a moment plays when the quiz does not say. */
export function defaultChoice(slot: SoundSlot): SoundChoice {
  const { music, madeFor } = SLOT_SOUNDS[slot];
  return music ? "none" : { builtIn: madeFor[0] };
}

/** The app's sound with `id`, if it has one that fits the moment: an effect for an effect, music for music. */
export function librarySoundFor(slot: SoundSlot, id: string): LibrarySound | undefined {
  const sound = BY_ID.get(id);
  return sound && (sound.kind === "music") === SLOT_SOUNDS[slot].music ? sound : undefined;
}

/**
 * What `slot` plays by the quiz's `sounds`: its own choice, or the default.
 * One of the app's sounds it does not have (from a newer build, or one that
 * does not fit the moment) plays the default, as a theme it does not have
 * shows the default look.
 */
export function choiceOf(sounds: QuizSounds, slot: SoundSlot): SoundChoice {
  const chosen = sounds.slots[slot];
  if (chosen === undefined) return defaultChoice(slot);
  if (typeof chosen === "object" && "builtIn" in chosen && !librarySoundFor(slot, chosen.builtIn)) return defaultChoice(slot);
  return chosen;
}

/** `sounds` with `slot` playing `choice`; a choice of the default is kept as no choice. */
export function withChoice(sounds: QuizSounds, slot: SoundSlot, choice: SoundChoice): QuizSounds {
  const slots = { ...sounds.slots };
  if (sameChoice(choice, defaultChoice(slot))) delete slots[slot];
  else slots[slot] = choice;
  return { ...sounds, slots };
}

export function sameChoice(a: SoundChoice, b: SoundChoice): boolean {
  return choiceKey(a) === choiceKey(b);
}

export function sameSounds(a: QuizSounds, b: QuizSounds): boolean {
  const chosen = (sounds: QuizSounds) => SOUND_SLOTS.map((slot) => (sounds.slots[slot] ? choiceKey(sounds.slots[slot]) : "")).join("\n");
  return a.effectsVolume === b.effectsVolume && a.musicVolume === b.musicVolume && chosen(a) === chosen(b);
}

/** A choice as text, one per choice: for comparing, and for the value of a select. */
export function choiceKey(choice: SoundChoice): string {
  if (choice === "none") return "none";
  return "builtIn" in choice ? `builtIn:${choice.builtIn}` : `file:${choice.file}`;
}

/** What a moment plays, found. */
export interface SoundSource {
  url: string;
  /**
   * One of the app's sounds: small, so it is read whole and plays to the
   * sample. A quiz's own file may be long, so it streams.
   */
  builtIn: boolean;
  /** The app's music: where the next round starts (`LibrarySound.loopMs`). */
  loopMs: number | null;
}

/**
 * The file `choice` plays for `slot`, or null for none. `fileUrl` finds a
 * quiz's own file; one it cannot find plays nothing.
 */
export function soundSource(slot: SoundSlot, choice: SoundChoice, fileUrl: (reference: string) => string | null): SoundSource | null {
  if (choice === "none") return null;
  if ("file" in choice) {
    const url = fileUrl(choice.file);
    return url ? { url, builtIn: false, loopMs: null } : null;
  }
  const sound = librarySoundFor(slot, choice.builtIn);
  const url = sound && libraryUrl(sound.id);
  return sound && url ? { url, builtIn: true, loopMs: sound.loopMs ?? null } : null;
}
