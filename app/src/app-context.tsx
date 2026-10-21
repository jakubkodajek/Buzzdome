import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api } from "./api";
import { translator, useLang, type Lang, type Translate } from "./i18n";
import { errorText } from "./messages";
import { createStepGuard, type StepGuard } from "./step-guard";
import type { Command, GameView } from "./types";

interface AppContextValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: Translate;
  /** Shows a failed action to the quizmaster. */
  notify: (message: string) => void;
  /** Lets one show step through per press or click, for the whole window (`useDispatch`). */
  steps: StepGuard;
}

const AppContext = createContext<AppContextValue | null>(null);

/** Language, translations and error reporting for one window. */
export function AppProvider({ notify, children }: { notify: (message: string) => void; children: ReactNode }) {
  const [lang, setLang] = useLang();
  const [steps] = useState(createStepGuard);
  const value = useMemo(() => ({ lang, setLang, t: translator(lang), notify, steps }), [lang, setLang, notify, steps]);
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

/** Tells the window's step guard whenever the show's step changes (`StepGuard.shown`). */
export function useStepShown(view: GameView | null) {
  const { steps } = useApp();
  const step = view?.stepNo ?? null;
  useEffect(() => {
    if (step !== null) steps.shown(step);
  }, [step, steps]);
}

/**
 * Sends a game command, the same way for every button, key and buzzer. A
 * command that moves the show on is dropped when it comes too soon after
 * the last one (`StepGuard`).
 */
export function useDispatch(): (command: Command) => void {
  const run = useRun();
  const { steps } = useApp();
  return useCallback(
    (command) => {
      const sent = steps.send(command, (seen) => api.dispatch(command, seen));
      if (sent) run(sent);
    },
    [run, steps],
  );
}
