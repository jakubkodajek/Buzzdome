// The fonts the font picker offers: the installed ones, read once per run.

import { useEffect } from "react";
import { api } from "../api";
import { createStore, useStore } from "../store";

/** Fonts most computers have, offered when the installed ones cannot be listed. */
export const COMMON_FONTS: readonly string[] = ["Arial", "Courier New", "Georgia", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana"];

const installed = createStore<readonly string[] | null>(null);
let asked = false;

/** The installed fonts, empty until they are read. */
export function useInstalledFonts(): readonly string[] {
  const fonts = useStore(installed);
  useEffect(() => {
    if (asked) return;
    asked = true;
    api.installedFonts().then(
      (names) => installed.set(names.length > 0 ? names : COMMON_FONTS),
      (error: unknown) => {
        console.error("installed fonts:", error);
        installed.set(COMMON_FONTS);
      },
    );
  }, []);
  return fonts ?? [];
}
