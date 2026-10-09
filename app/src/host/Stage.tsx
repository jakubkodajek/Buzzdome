import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import "./slide.css";

const STAGE_W = 1920;
const STAGE_H = 1080;

/**
 * A fixed 1920x1080 canvas scaled to fit the element it is placed in (styled
 * by `className`), letterboxed. Sizes inside are in stage pixels.
 */
export function Stage({ className, children }: { className: string; children: ReactNode }) {
  const frame = useRef<HTMLDivElement>(null);
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
      <div className="stage" style={{ width: STAGE_W, height: STAGE_H, transform: `translate(-50%, -50%) scale(${scale})` }}>
        {children}
      </div>
    </div>
  );
}
