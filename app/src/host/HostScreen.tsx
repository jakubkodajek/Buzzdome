import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { AppProvider, useApp, useRun, useStepShown } from "../app-context";
import { ErrorBoundary } from "../error-boundary";
import { useBackendEvent, useClock, useGameView, useMediaRoot, useQuiz, useWindowTitle } from "../hooks";
import { CheckIcon, CrossIcon } from "../icons";
import { useProjectorCloseKeys, useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import type { Notice } from "../messages";
import type { TonedNotice } from "../notice";
import { slideContext } from "../quiz";
import { ThemeContext, ThemesProvider, useResolvedTheme } from "../theme/context";
import { screenOf } from "../theme/themes";
import { MediaOverlay } from "./MediaOverlay";
import { Leaderboard, ranked, SlideScreen } from "./SlideScreen";
import { Stage, StageOverlay } from "./Stage";
import "./host.css";

// The projector has nowhere to show errors; the Director shows them.
export const reportToDirector = (problem: string | Notice) => {
  const notice = typeof problem === "string" ? { text: problem } : problem;
  void api.reportHostProblem(notice).catch(() => console.error(notice));
};

/** A projector message for the Director, which shows it as a failure. */
const showOnDirector = ({ text, detail }: TonedNotice) => reportToDirector({ text, detail });

/** The projector. Renders the game state; owns nothing. */
export function HostScreen() {
  return (
    <AppProvider show={showOnDirector}>
      <ThemesProvider>
        <HostLayout />
      </ThemesProvider>
    </AppProvider>
  );
}

function HostLayout() {
  const { t } = useApp();
  const run = useRun();
  const view = useGameView();
  const quiz = useQuiz();
  const clock = useClock(view);
  const mediaRoot = useMediaRoot();
  const theme = useResolvedTheme(quiz?.theme ?? null);
  const close = useCallback(() => run(api.closeHostWindow()), [run]);
  useWindowTitle(t("projector"), t("appTitle"));
  useStepShown(view);
  useShortcuts(view);
  useProjectorCloseKeys(close, view !== null);
  const verdict = useVerdict();

  return (
    <MediaRootContext value={mediaRoot}>
      <ThemeContext value={theme}>
        <Stage className="host-viewport" screen={view ? screenOf(view) : "default"} slide={view?.slide ?? null} onHold={view?.onHold} bleed>
          {view && (
            <>
              {/* A slide that fails to render leaves the stage as it is and tells the Director; the next slide tries afresh. */}
              <ErrorBoundary key={view.slideIndex} fallback={() => null} onError={(message) => reportToDirector(t("slideNotShown", { message }))}>
                <SlideScreen view={view} context={slideContext(quiz, view.slideIndex)} clock={clock} />
              </ErrorBoundary>
              {verdict && (
                // A new element for each verdict, so a second one in a row plays from its start.
                <div key={verdict.n} className={`verdict verdict-${verdict.cue}`}>
                  {verdict.cue === "correct" ? <CheckIcon /> : <CrossIcon />}
                </div>
              )}
              {view.leaderboardVisible && (
                <StageOverlay screen="leaderboard">
                  <div className="center-slide">
                    <h1 className="slide-title small">{t("standings")}</h1>
                    <Leaderboard players={ranked(view)} />
                  </div>
                </StageOverlay>
              )}
            </>
          )}
          <MediaOverlay />
        </Stage>
      </ThemeContext>
    </MediaRootContext>
  );
}

/** A judgement shown: its cue, and how many came before it, so each one is drawn afresh. */
interface Verdict {
  cue: "correct" | "wrong";
  n: number;
}

/** A big check or cross for a moment after each answer the quizmaster judges. */
function useVerdict(): Verdict | null {
  const [verdict, setVerdict] = useState<Verdict | null>(null);
  useBackendEvent("sound-cue", (cue) => {
    if (cue === "correct" || cue === "wrong") setVerdict((last) => ({ cue, n: (last?.n ?? 0) + 1 }));
  });
  useEffect(() => {
    if (!verdict) return;
    const timer = setTimeout(() => setVerdict(null), 1600);
    return () => clearTimeout(timer);
  }, [verdict]);
  return verdict;
}
