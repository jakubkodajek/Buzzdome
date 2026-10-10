import { useMemo, useState } from "react";
import { DEFAULT_THEME_ID } from "../bindings/Constants";
import { useLatest } from "../hooks";
import { useResolvedTheme } from "../theme/context";
import type { Theme } from "../types";

/** What the Design tab's theme gallery works with. */
export interface ThemeChoice {
  /** Id of the quiz's theme, the default one's when it has none. */
  current: string;
  /** Gives the quiz a theme, as an edit that can be undone. */
  pick: (id: string) => void;
  /** Shows a theme on the slide canvas only, while it is pointed at; null ends that. */
  preview: (id: string | null) => void;
}

/**
 * The quiz's theme, the theme of the slide canvas (one being previewed in
 * the gallery, else the quiz's) and the gallery's choice. `setTheme` makes
 * the edit.
 */
export function useThemeChoice(
  quizTheme: string | null,
  setTheme: (id: string) => void,
): { theme: Theme | null; canvasTheme: Theme | null; choice: ThemeChoice } {
  const [previewed, setPreviewed] = useState<string | null>(null);
  const theme = useResolvedTheme(quizTheme);
  const canvasTheme = useResolvedTheme(previewed ?? quizTheme);
  const pick = useLatest(setTheme);
  const choice = useMemo(
    () => ({ current: quizTheme ?? DEFAULT_THEME_ID, pick: (id: string) => pick.current(id), preview: setPreviewed }),
    [quizTheme, pick],
  );
  return { theme, canvasTheme, choice };
}
