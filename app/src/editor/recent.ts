// Quiz files opened or saved lately, for the FILE menu. Kept per computer:
// losing the list loses nothing else.

import { readPreference, writePreference } from "../preferences";

const KEY = "quiz-show.recent-files";
const KEEP = 10;

export function readRecentFiles(): string[] {
  try {
    const list: unknown = JSON.parse(readPreference(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((item): item is string => typeof item === "string").slice(0, KEEP) : [];
  } catch {
    return [];
  }
}

/** Puts `path` first in the list and returns the new list. */
export function rememberRecentFile(path: string): string[] {
  const list = [path, ...readRecentFiles().filter((item) => item !== path)].slice(0, KEEP);
  writePreference(KEY, JSON.stringify(list));
  return list;
}
