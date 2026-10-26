// The Director's look: light, dark for a darkened hall, or as the system
// is set. Kept per computer (preferences.ts); the projector keeps its theme.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { api } from "../api";
import { thrownText } from "../messages";
import { readPreference, writePreference } from "../preferences";

export const LOOKS = ["light", "dark", "system"] as const;
export type Look = (typeof LOOKS)[number];

const LOOK_KEY = "quiz-show.director-look";

/** The look `stored`, or light, as before there was a choice. */
export function readLook(stored: string | null): Look {
  return LOOKS.find((look) => look === stored) ?? "light";
}

/** Whether the Director shows dark in `look`, the system being dark or not. */
export function showsDark(look: Look, systemDark: boolean): boolean {
  return look === "dark" || (look === "system" && systemDark);
}

const SYSTEM_DARK = "(prefers-color-scheme: dark)";

function systemIsDark(): boolean {
  return window.matchMedia(SYSTEM_DARK).matches;
}

function onSystemChange(listener: () => void): () => void {
  const query = window.matchMedia(SYSTEM_DARK);
  query.addEventListener("change", listener);
  return () => query.removeEventListener("change", listener);
}

/** The window's title bar, where the system draws it (Windows): null as the system has it. */
function setTitleBar(theme: "dark" | null) {
  getCurrentWindow()
    .setTheme(theme)
    .catch((error: unknown) => api.logProblem(thrownText(error)).catch(() => undefined));
}

export interface DirectorLook {
  look: Look;
  setLook: (look: Look) => void;
  /** Whether the Director shows dark now. */
  dark: boolean;
}

export function useDirectorLook(): DirectorLook {
  const [look, setLookState] = useState<Look>(() => readLook(readPreference(LOOK_KEY)));
  const systemDark = useSyncExternalStore(onSystemChange, systemIsDark);
  const setLook = useCallback((next: Look) => {
    setLookState(next);
    writePreference(LOOK_KEY, next);
  }, []);
  // Dark only while the dark Director is on screen: the system's own otherwise, as before.
  useEffect(() => {
    if (look !== "dark") return;
    setTitleBar("dark");
    return () => setTitleBar(null);
  }, [look]);
  return { look, setLook, dark: showsDark(look, systemDark) };
}
