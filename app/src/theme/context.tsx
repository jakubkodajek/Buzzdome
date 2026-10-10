import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api } from "../api";
import { useReport } from "../app-context";
import { useLatest } from "../hooks";
import type { Theme } from "../types";
import { registerFonts } from "./fonts";
import { pickTheme } from "./themes";

/** The installed themes (`list_themes`), empty until they are loaded. */
const ThemesContext = createContext<readonly Theme[]>([]);

/** Loads the installed themes once for the window and registers their fonts. */
export function ThemesProvider({ children }: { children: ReactNode }) {
  const report = useLatest(useReport());
  const [themes, setThemes] = useState<readonly Theme[]>([]);
  useEffect(() => {
    api.listThemes().then(
      (list) => {
        list.forEach(registerFonts);
        setThemes(list);
      },
      (e: unknown) => report.current(e),
    );
  }, [report]);
  return <ThemesContext value={themes}>{children}</ThemesContext>;
}

export function useThemes(): readonly Theme[] {
  return useContext(ThemesContext);
}

/** The look of the stages below it (`Stage`); none until the themes are loaded. */
export const ThemeContext = createContext<Theme | null>(null);

export function useTheme(): Theme | null {
  return useContext(ThemeContext);
}

/** The installed theme `id` (null: the default; see `pickTheme`). */
export function useResolvedTheme(id: string | null): Theme | null {
  const themes = useThemes();
  return useMemo(() => pickTheme(themes, id), [themes, id]);
}
