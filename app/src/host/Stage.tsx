import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTheme } from "../theme/context";
import { backgroundStyle, themeStyle, type ScreenKey } from "../theme/themes";
import "./slide.css";

const STAGE_W = 1920;
const STAGE_H = 1080;

/**
 * A fixed 1920x1080 canvas scaled to fit the element it is placed in (styled
 * by `className`), letterboxed. Sizes inside are in stage pixels. It is drawn
 * in the theme of `ThemeContext`, with that theme's background for `screen`.
 */
export function Stage({ className, screen, children }: { className: string; screen: ScreenKey; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null);
  const theme = useTheme();
  const look = useMemo(() => themeStyle(theme), [theme]);
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
        <Backdrop screen={screen} />
        {children}
      </div>
    </div>
  );
}

/** The theme's background for `screen`, filling its parent; a slide's own background covers it. */
export function Backdrop({ screen }: { screen: ScreenKey }) {
  const theme = useTheme();
  return <div className="backdrop" style={backgroundStyle(theme, screen)} />;
}
