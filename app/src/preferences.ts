// Settings kept per computer in the web view's storage. Losing one loses
// nothing else, so reading and writing never fail: without storage a value
// lasts until the window closes.

/** The stored value of `key`, or null when there is none or no storage. */
export function readPreference(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writePreference(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Kept only by the caller's state, until the window closes.
  }
}
