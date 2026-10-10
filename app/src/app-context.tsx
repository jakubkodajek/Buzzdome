import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";
import { api } from "./api";
import { translator, useLang, type Lang, type Translate } from "./i18n";
import { errorText } from "./messages";
import type { Command } from "./types";

interface AppContextValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: Translate;
  /** Shows a failed action to the quizmaster. */
  notify: (message: string) => void;
}

const AppContext = createContext<AppContextValue | null>(null);

/** Language, translations and error reporting for one window. */
export function AppProvider({ notify, children }: { notify: (message: string) => void; children: ReactNode }) {
  const [lang, setLang] = useLang();
  const value = useMemo(() => ({ lang, setLang, t: translator(lang), notify }), [lang, setLang, notify]);
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp(): AppContextValue {
  const value = useContext(AppContext);
  if (!value) throw new Error("useApp() needs an <AppProvider> above it");
  return value;
}

/** Shows a failed backend call to the quizmaster, in their language. */
export function useReport(): (error: unknown) => void {
  const { t, notify } = useApp();
  return useCallback((error) => notify(errorText(t, error)), [t, notify]);
}

/** Runs a backend call; a failure is reported to the quizmaster, not thrown. */
export function useRun(): (call: Promise<unknown>) => void {
  const report = useReport();
  return useCallback((call) => void call.catch(report), [report]);
}

/** Sends a game command, the same way for every button, key and buzzer. */
export function useDispatch(): (command: Command) => void {
  const run = useRun();
  return useCallback((command) => run(api.dispatch(command)), [run]);
}
