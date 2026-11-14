import { readPreference, writePreference } from "../preferences";

/** The most on-screen buzzers, and how many there are until the quizmaster picks. */
export const BUZZER_COUNT = { min: 1, max: 100, initial: 6 } as const;

const COUNT_KEY = "quiz-show.virtual-buzzers";

/** How many buzzers `text` asks for, within the bounds; `fallback` when it names no number. */
export function buzzerCount(text: string | null, fallback: number = BUZZER_COUNT.initial): number {
  const n = text === null || text.trim() === "" ? NaN : Math.round(Number(text));
  return Number.isFinite(n) ? Math.max(BUZZER_COUNT.min, Math.min(BUZZER_COUNT.max, n)) : fallback;
}

/** The count picked last time on this computer, null when none was. */
export function readPickedBuzzerCount(): number | null {
  const text = readPreference(COUNT_KEY);
  return text === null ? null : buzzerCount(text);
}

/**
 * The count picked last time on this computer; until one is picked, one
 * on-screen buzzer for each registered player (the usual six with none).
 */
export function readBuzzerCount(registered: number = 0): number {
  return buzzerCount(readPreference(COUNT_KEY), registered > 0 ? buzzerCount(String(registered)) : BUZZER_COUNT.initial);
}

export function writeBuzzerCount(count: number): void {
  writePreference(COUNT_KEY, String(count));
}
