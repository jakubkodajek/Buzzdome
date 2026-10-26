import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { AppProvider, useApp, useRun } from "../app-context";
import { ErrorBoundary } from "../error-boundary";
import { useBackendEvent, useClock, useGameView, useMediaRoot, useQuiz } from "../hooks";
import { CheckIcon, CrossIcon } from "../icons";
import { useProjectorCloseKeys, useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import { slideContext } from "../quiz";
import { ThemeContext, ThemesProvider, useResolvedTheme } from "../theme/context";
import { screenOf } from "../theme/themes";
import { MediaOverlay } from "./MediaOverlay";
import { Leaderboard, ranked, SlideScreen } from "./SlideScreen";
import { Stage, StageOverlay } from "./Stage";
import "./host.css";

// The projector has nowhere to show errors; the Director shows them.
export const reportToDirector = (message: string) => void api.reportHostProblem(message).catch(() => console.error(message));

/** The projector. Renders the game state; owns nothing. */
export function HostScreen() {
  return (
    <AppProvider notify={reportToDirector}>
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
  useShortcuts(view);
  useProjectorCloseKeys(close);
  const verdict = useVerdict();

  return (
    <MediaRootContext value={mediaRoot}>
      <ThemeContext value={theme}>
        <Stage className="host-viewport" screen={view ? screenOf(view) : "default"} slide={view?.slide ?? null} onHold={view?.onHold}>
          {view && (
            <>
              {/* A slide that fails to render leaves the stage as it is and tells the Director; the next slide tries afresh. */}
              <ErrorBoundary key={view.slideIndex} fallback={() => null} onError={(message) => reportToDirector(t("slideNotShown", { message }))}>
                <SlideScreen view={view} context={slideContext(quiz, view.slideIndex)} clock={clock} />
              </ErrorBoundary>
              {verdict && <div className={`verdict verdict-${verdict}`}>{verdict === "correct" ? <CheckIcon /> : <CrossIcon />}</div>}
              {view.leaderboardVisible && (
                <StageOverlay screen="leaderboard">
                  <Leaderboard players={ranked(view)} />
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

/** A big check or cross for a moment after the quizmaster judges an answer. */
function useVerdict(): "correct" | "wrong" | null {
  const [verdict, setVerdict] = useState<"correct" | "wrong" | null>(null);
  useBackendEvent("sound-cue", (cue) => {
    if (cue === "correct" || cue === "wrong") setVerdict(cue);
  });
  useEffect(() => {
    if (!verdict) return;
    const timer = setTimeout(() => setVerdict(null), 1600);
    return () => clearTimeout(timer);
  }, [verdict]);
  return verdict;
}
