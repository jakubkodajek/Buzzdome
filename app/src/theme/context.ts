import { createContext, useContext, useMemo } from "react";
import { resolveTheme, type Theme } from "./themes";

/** The look of the stages below it (`Stage`); the default theme when none is given. */
export const ThemeContext = createContext<Theme>(resolveTheme(null));

export function useTheme(): Theme {
  return useContext(ThemeContext);
}

/** The installed theme `id` (null: the default), resolved once per id. */
export function useResolvedTheme(id: string | null): Theme {
  return useMemo(() => resolveTheme(id), [id]);
}
