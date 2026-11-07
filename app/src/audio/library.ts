// The show's sounds (spec SN-1 to SN-4): what each moment of the show plays,
// from the app's own library (made by `library/make_sounds.py`), from a
// sound file of the quiz, or nothing.

import { LAST_SECONDS_MS, MUSIC_SLOTS, SLOT_PLAYABLE_IDS, SLOT_SOUND_IDS, SOUND_SLOTS } from "../bindings/Constants";
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
  /**
   * The app's sounds made for the moment (`SoundSlot::made_for`). An
   * effect's first one is its default; music plays none unless the quiz says.
   */
  madeFor: readonly string[];
}

const LABELS: Record<SoundSlot, StringKey> = {
  clockStart: CUE_LABEL.clockStart,
  countdown: "soundCountdown",
  buzz: CUE_LABEL.buzz,
  correct: CUE_LABEL.correct,
  wrong: CUE_LABEL.wrong,
  timeUp: CUE_LABEL.timeUp,
  reveal: CUE_LABEL.reveal,
  leaderboard: CUE_LABEL.leaderboard,
  place: CUE_LABEL.place,
  winner: CUE_LABEL.winner,
  lobbyMusic: "soundLobbyMusic",
  betweenMusic: "soundBetweenMusic",
  clockMusic: "soundClockMusic",
  buildUpMusic: "soundBuildUpMusic",
  winnerMusic: "soundWinnerMusic",
};

/** Each moment of the show: its name and the app's sounds made for it. */
const slotSounds = (slot: SoundSlot): SlotSounds => ({ label: LABELS[slot], madeFor: SLOT_SOUND_IDS[slot] });
export const SLOT_SOUNDS = Object.fromEntries(SOUND_SLOTS.map((slot) => [slot, slotSounds(slot)])) as Record<SoundSlot, SlotSounds>;

/** What `slot` is called. */
export function slotLabel(t: Translate, slot: SoundSlot): string {
  return t(SLOT_SOUNDS[slot].label, { s: LAST_SECONDS_MS / 1000 });
}

const MUSIC: ReadonlySet<SoundSlot> = new Set(MUSIC_SLOTS);

/** Music plays over and over and fades in and out; an effect plays once. */
export function isMusic(slot: SoundSlot): boolean {
  return MUSIC.has(slot);
}

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
  return isMusic(slot) ? "none" : { builtIn: SLOT_SOUNDS[slot].madeFor[0] };
}

/**
 * The app's sound with `id`, if it plays at the moment: one quiz-core says
 * fits it (`SoundSlot::playable`), an effect for an effect, music for music.
 */
export function librarySoundFor(slot: SoundSlot, id: string): LibrarySound | undefined {
  return (SLOT_PLAYABLE_IDS[slot] as readonly string[]).includes(id) ? BY_ID.get(id) : undefined;
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
