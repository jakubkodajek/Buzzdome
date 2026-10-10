import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../app-context";
import { Timer } from "../host/Timer";
import { useTheme } from "../theme/context";
import { themeStyle } from "../theme/themes";
import { TIMER_DESIGNS, TIMER_LABEL } from "../timers";
import type { TimerDesign } from "../types";
import type { PropertyRow } from "./property-rows";

/** Each design is drawn in the quiz's theme with this much time left. */
const SAMPLE = { seconds: 12, fraction: 0.6, label: "3/12" };

interface TimerGalleryProps {
  /** The `timer` row of the selected questions (style-properties.tsx), which this sets as the panel does. */
  row: PropertyRow;
  /** The selected questions differ in it: none is pressed. */
  mixed: boolean;
}

/** The designs of a question's clock to choose from in the ribbon, drawn as the projector draws them. */
export function TimerGallery({ row, mixed }: TimerGalleryProps) {
  const { t } = useApp();
  const theme = useTheme();
  const look = useMemo(() => (theme ? themeStyle(theme) : undefined), [theme]);
  const value = row.value;
  if (value?.kind !== "choice") return null;
  return (
    <div className="theme-gallery" style={look}>
      {TIMER_DESIGNS.map((design) => (
        <button
          key={design}
          type="button"
          className="theme-tile"
          title={`${row.label}: ${t(TIMER_LABEL[design])}`}
          aria-pressed={!mixed && value.value === design}
          onClick={() => value.set(design)}
        >
          <TimerSample design={design} />
          <span>{t(TIMER_LABEL[design])}</span>
        </button>
      ))}
    </div>
  );
}

/** A clock in stage pixels shrunk to fit its tile, whatever its design's size. */
function TimerSample({ design }: { design: TimerDesign }) {
  const frame = useRef<HTMLDivElement>(null);
  const clock = useRef<HTMLDivElement>(null);
  // Hidden until measured, so it never flashes at full size.
  const [scale, setScale] = useState(0);
  useLayoutEffect(() => {
    const box = frame.current;
    const drawn = clock.current;
    if (!box || !drawn) return;
    // Measured again once the theme's font has loaded and widened the digits.
    const observer = new ResizeObserver(() =>
      setScale(SAMPLE_FILL * Math.min(box.clientWidth / drawn.offsetWidth, box.clientHeight / drawn.offsetHeight)),
    );
    observer.observe(drawn);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={frame} className="timer-sample">
      <div ref={clock} className="timer-fit" style={{ transform: `translate(-50%, -50%) scale(${scale})` }}>
        <Timer design={design} {...SAMPLE} />
      </div>
    </div>
  );
}

/** How much of its tile a clock fills, leaving a margin. */
const SAMPLE_FILL = 0.85;
