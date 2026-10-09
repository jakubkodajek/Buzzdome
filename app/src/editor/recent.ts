// Quiz files opened or saved lately, for the FILE menu. Kept per computer in
// the web view's storage: losing the list loses nothing else.

const KEY = "quiz-show.recent-files";
const KEEP = 10;

export function readRecentFiles(): string[] {
  try {
    const list: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((item): item is string => typeof item === "string").slice(0, KEEP) : [];
  } catch {
    return [];
  }
}

/** Puts `path` first in the list and returns the new list. */
export function rememberRecentFile(path: string): string[] {
  const list = [path, ...readRecentFiles().filter((item) => item !== path)].slice(0, KEEP);
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // The list then lasts until the window closes.
  }
  return list;
}
