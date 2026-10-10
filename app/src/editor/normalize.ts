// Evening out the loudness of a quiz's sounds (SN-9): the target the
// quizmaster chose, and measuring the part of a sound that plays.

import { decodeAudioFile, SoundTooLong } from "../audio/decode";
import { LOUDNESS_TARGETS, normalization, type Loudness, type LoudnessStatus, type LoudnessTarget, type Normalization } from "../audio/loudness";
import { measureParts } from "../audio/measure";
import type { StringKey } from "../i18n";
import { readPreference, writePreference } from "../preferences";
import type { SoundClip } from "../types";

const TARGET_KEY = "quiz-show.loudness-target";

/** The target last chosen on this computer; the first (Standard) at first. */
export function readLoudnessTarget(): LoudnessTarget {
  const stored = Number(readPreference(TARGET_KEY));
  return LOUDNESS_TARGETS.find((target) => target === stored) ?? LOUDNESS_TARGETS[0];
}

export function writeLoudnessTarget(target: LoudnessTarget): void {
  writePreference(TARGET_KEY, String(target));
}

/** The loudness of the part of `sound` that plays, from its file at `url`. */
export async function measureSound(url: string, sound: Pick<SoundClip, "part">): Promise<Loudness> {
  const [loudness] = await measureParts(await decodeAudioFile(url), [sound.part]);
  return loudness;
}

/** Why a sound was not measured: its file could not be read, or is too long to read whole. */
export type UnreadSound = "failed" | "tooLong";

export function unreadSound(error: unknown): UnreadSound {
  return error instanceof SoundTooLong ? "tooLong" : "failed";
}

/** How each target is offered. */
export const LOUDNESS_TARGET_KEYS: Record<LoudnessTarget, StringKey> = {
  [-16]: "loudnessStandard",
  [-18]: "loudnessQuieter",
  [-23]: "loudnessBroadcast",
};

/** What became of a sound, or why it was not measured (worded with `minutes`, see `MAX_DECODED_MINUTES`). */
export const LOUDNESS_STATUS_KEYS: Record<LoudnessStatus | UnreadSound, StringKey> = {
  changed: "loudnessChanged",
  atTarget: "loudnessAtTarget",
  limited: "loudnessLimited",
  silent: "loudnessSilent",
  tooShort: "loudnessTooShort",
  failed: "loudnessFailed",
  tooLong: "loudnessTooLong",
};

export interface GainChange {
  gainDb: number | null;
  status: LoudnessStatus;
  message: StringKey;
}

/** What normalizing a sound whose gain is `currentDb` and that measures as `loudness` to `target` does (`gainChange`). */
export function normalizedGain(currentDb: number, loudness: Loudness, target: number): GainChange {
  return gainChange(currentDb, normalization(loudness, target));
}

/**
 * The gain of a normalized sound once it plays another part, measured as
 * `loudness`: what normalizing it gives, or the gain it had when the part
 * cannot be measured (silent or too short).
 */
export function gainAfterTrim(currentDb: number, loudness: Loudness, target: number): number {
  return normalizedGain(currentDb, loudness, target).gainDb ?? currentDb;
}

/**
 * What normalizing a sound whose gain is `currentDb` does: the gain to set,
 * or null when it stays (nothing to measure, or the gain is right already),
 * the status that says so ("changed" only when the gain does change), and
 * the message for the quizmaster.
 */
export function gainChange(currentDb: number, { gainDb, status }: Normalization): GainChange {
  if (status === "silent" || status === "tooShort") return { gainDb: null, status, message: LOUDNESS_STATUS_KEYS[status] };
  if (gainDb === currentDb) {
    const same = status === "limited" ? "limited" : "atTarget";
    return { gainDb: null, status: same, message: LOUDNESS_STATUS_KEYS[same] };
  }
  return status === "limited" ? { gainDb, status, message: "normalizeLimited" } : { gainDb, status: "changed", message: "normalizeDone" };
}
