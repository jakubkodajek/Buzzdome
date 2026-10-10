import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { StageBackground, type ScreenMode } from "../slide-media";
import { resolveBackground } from "../theme/background";
import { useResolvedTheme, useTheme } from "../theme/context";
import { themeStyle } from "../theme/themes";
import type { ScreenKey, Slide } from "../types";
import "./slide.css";

const STAGE_W = 1920;
const STAGE_H = 1080;

interface StageProps {
  /** Styles the element the stage fits into. */
  className: string;
  /** The screen shown, for the theme's background (TH-5). */
  screen: ScreenKey;
  /** The slide shown, for its own background; none between slides. */
  slide: Slide | null;
  /** Where the stage is drawn, for how a background video plays. */
  mode?: ScreenMode;
  /** The show holds: a synced background video pauses (BG-4). */
  onHold?: boolean;
  /** The editor plays the slide's background video. */
  playingBackground?: boolean;
  children: ReactNode;
}

/**
 * A fixed 1920x1080 canvas scaled to fit the element it is placed in,
 * letterboxed. Sizes inside are in stage pixels. It is drawn in the theme of
 * `ThemeContext`, behind it the background `resolveBackground` picks.
 */
export function Stage({ className, screen, slide, mode, onHold, playingBackground, children }: StageProps) {
  const frame = useRef<HTMLDivElement>(null);
  const theme = useTheme();
  const builtIn = useResolvedTheme(null);
  const look = useMemo(() => (theme ? themeStyle(theme) : undefined), [theme]);
  // Hidden until the frame is measured, so it never flashes at full size.
  const [scale, setScale] = useState(0);
  useLayoutEffect(() => {
    const element = frame.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setScale(Math.min(width / STAGE_W, height / STAGE_H));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={frame} className={className}>
      <div className="stage" style={{ ...look, width: STAGE_W, height: STAGE_H, transform: `translate(-50%, -50%) scale(${scale})` }}>
        <StageBackground layers={resolveBackground(slide, theme, screen, builtIn)} mode={mode} onHold={onHold} playing={playingBackground} />
        {children}
      </div>
    </div>
  );
}

/**
 * A screen drawn over the stage's slide, such as the leaderboard the
 * quizmaster shows mid-question, with the theme's background for `screen`.
 * Backgrounds are only drawn here and in `Stage`.
 */
export function StageOverlay({ screen, children }: { screen: ScreenKey; children: ReactNode }) {
  const theme = useTheme();
  const builtIn = useResolvedTheme(null);
  return (
    <div className="overlay">
      <StageBackground layers={resolveBackground(null, theme, screen, builtIn)} />
      {children}
    </div>
  );
}
