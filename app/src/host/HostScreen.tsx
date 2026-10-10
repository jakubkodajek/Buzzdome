import { useEffect, useState } from "react";
import { AppProvider } from "../app-context";
import { useBackendEvent, useClock, useGameView, useMediaRoot, useQuiz } from "../hooks";
import { CheckIcon, CrossIcon } from "../icons";
import { useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import { slideContext } from "../quiz";
import { SlideBackground } from "../slide-media";
import { ThemeContext, useResolvedTheme } from "../theme/context";
import { screenOf } from "../theme/themes";
import { Leaderboard, ranked, SlideScreen } from "./SlideScreen";
import { Backdrop, Stage } from "./Stage";
import "./host.css";

// The projector has nowhere to show errors; the Director reports them.
const logError = (message: string) => console.error(message);

/** The projector. Renders the game state; owns nothing. */
export function HostScreen() {
  return (
    <AppProvider notify={logError}>
      <HostLayout />
    </AppProvider>
  );
}

function HostLayout() {
  const view = useGameView();
  const quiz = useQuiz();
  const clock = useClock(view);
  const mediaRoot = useMediaRoot();
  const theme = useResolvedTheme(quiz?.theme ?? null);
  useShortcuts(view);
  const verdict = useVerdict();

  return (
    <MediaRootContext value={mediaRoot}>
      <ThemeContext value={theme}>
        <Stage className="host-viewport" screen={view ? screenOf(view) : "default"}>
          {view && (
            <>
              {view.slide && <SlideBackground slide={view.slide} onHold={view.onHold} />}
              <SlideScreen view={view} context={slideContext(quiz, view.slideIndex)} clock={clock} />
              {verdict && <div className={`verdict verdict-${verdict}`}>{verdict === "correct" ? <CheckIcon /> : <CrossIcon />}</div>}
              {view.leaderboardVisible && (
                <div className="overlay">
                  <Backdrop screen="leaderboard" />
                  <Leaderboard players={ranked(view)} />
                </div>
              )}
            </>
          )}
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
