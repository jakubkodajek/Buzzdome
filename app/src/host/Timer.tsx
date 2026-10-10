// A question's clock on the projector, in the design the slide's style
// picks (quiz_core::TimerDesign): the seconds left, how much of the answer
// time is left, and the question's place in the quiz.

import { useId, type ComponentType } from "react";
import { minutesAndSeconds } from "../format";
import { DEFAULT_TIMER } from "../timers";
import type { TimerDesign } from "../types";
import "./timer.css";

/** Seconds left from which the clock urges the players on. */
const URGENT_SECONDS = 5;

export interface TimerProps {
  /** The slide's design; none is `DEFAULT_TIMER`. */
  design: TimerDesign | undefined;
  /** Whole seconds left; null for a question without a time limit. */
  seconds: number | null;
  /** How much of the answer time is left, 0 to 1. */
  fraction: number;
  /** The question's place in the quiz, such as 3/12. */
  label: string;
}

export function Timer({ design = DEFAULT_TIMER, seconds, fraction, label }: TimerProps) {
  const Design = DESIGNS[design];
  const urgent = seconds !== null && seconds <= URGENT_SECONDS;
  return (
    <div className={`timer timer-${design}${urgent ? " urgent" : ""}`} data-element="timer">
      <Design left={Math.max(0, Math.min(1, fraction))} shown={seconds === null ? "∞" : String(seconds)} seconds={seconds} label={label} />
    </div>
  );
}

interface Drawn {
  /** How much of the answer time is left, 0 to 1. */
  left: number;
  /** The seconds as shown, ∞ without a time limit. */
  shown: string;
  seconds: number | null;
  label: string;
}

const DESIGNS: Record<TimerDesign, ComponentType<Drawn>> = {
  ring: Ring,
  bar: Bar,
  digital: Digital,
  hourglass: Hourglass,
  stopwatch: Stopwatch,
};

/** The length of a circle of radius `r`, for drawing a part of it with a dash. */
const circumference = (r: number) => 2 * Math.PI * r;

function Ring({ left, shown, label }: Drawn) {
  const r = 70;
  return (
    <>
      <svg viewBox="0 0 180 180">
        <circle cx="90" cy="90" r="86" className="timer-face" />
        <circle cx="90" cy="90" r={r} className="ring-track" />
        <circle
          cx="90"
          cy="90"
          r={r}
          className="ring-fill"
          strokeDasharray={circumference(r)}
          strokeDashoffset={circumference(r) * (1 - left)}
          transform="rotate(-90 90 90)"
        />
      </svg>
      <span className="timer-seconds ring-seconds">{shown}</span>
      <span className="ring-label">{label}</span>
    </>
  );
}

function Bar({ left, shown, label }: Drawn) {
  return (
    <>
      <span className="timer-seconds">{shown}</span>
      <span className="bar-gauge">
        <span className="bar-track">
          <span className="bar-fill" style={{ width: `${left * 100}%` }} />
        </span>
        <span className="timer-label">{label}</span>
      </span>
    </>
  );
}

function Digital({ left, seconds, label }: Drawn) {
  const digits = seconds === null ? "-:--" : minutesAndSeconds(seconds);
  return (
    <>
      <span className="digital-display">
        {/* The unlit segments behind the lit ones, as on a real display. */}
        <span className="digital-unlit" aria-hidden="true">
          {digits.replace(/\d/g, "8")}
        </span>
        <span className="timer-seconds digital-lit">{digits}</span>
      </span>
      <span className="digital-foot">
        <span className="digital-line">
          <span className="digital-fill" style={{ width: `${left * 100}%` }} />
        </span>
        <span className="timer-label">{label}</span>
      </span>
    </>
  );
}

function Hourglass({ left, shown, label }: Drawn) {
  // Each clock its own clips, the editor draws many on one page; without
  // the id's delimiters, which a url() reference may not take.
  const id = `hourglass${useId().replace(/[^\w-]/g, "")}`;
  // The sand's surface in each bulb: the top one empties from above, the bottom one fills from below.
  const [top, neck, bottom] = [24, 90, 156];
  const height = neck - top;
  return (
    <>
      <svg viewBox="0 0 120 180" className="hourglass">
        <defs>
          <clipPath id={`${id}-top`}>
            <rect x="0" y={neck - height * left} width="120" height={height * left} />
          </clipPath>
          <clipPath id={`${id}-bottom`}>
            <rect x="0" y={bottom - height * (1 - left)} width="120" height={height * (1 - left)} />
          </clipPath>
        </defs>
        <path d={TOP_BULB} className="hourglass-glass" />
        <path d={BOTTOM_BULB} className="hourglass-glass" />
        <path d={TOP_BULB} className="hourglass-sand" clipPath={`url(#${id}-top)`} />
        <path d={BOTTOM_BULB} className="hourglass-sand" clipPath={`url(#${id}-bottom)`} />
        {left > 0 && left < 1 && <rect x="58.5" y={neck} width="3" height={bottom - neck} className="hourglass-stream" />}
        <rect x="10" y="8" width="100" height="16" rx="5" className="hourglass-cap" />
        <rect x="10" y="156" width="100" height="16" rx="5" className="hourglass-cap" />
      </svg>
      <span className="timer-readout">
        <span className="timer-seconds">{shown}</span>
        <span className="timer-label">{label}</span>
      </span>
    </>
  );
}

const TOP_BULB = "M22 24 H98 C98 58 68 74 62 90 H58 C52 74 22 58 22 24 Z";
const BOTTOM_BULB = "M58 90 H62 C68 106 98 122 98 156 H22 C22 122 52 106 58 90 Z";

function Stopwatch({ left, shown, label }: Drawn) {
  // A slice of half the face's radius drawn with a dash as wide as the face: the time left, from twelve o'clock.
  const r = 30;
  return (
    <>
      <svg viewBox="0 0 140 160" className="stopwatch">
        <rect x="60" y="2" width="20" height="14" rx="3" className="stopwatch-crown" />
        <rect x="64" y="14" width="12" height="10" className="stopwatch-crown" />
        <circle cx="70" cy="88" r="66" className="timer-face" />
        <circle
          cx="70"
          cy="88"
          r={r}
          className="stopwatch-slice"
          strokeWidth={r * 2}
          strokeDasharray={circumference(r)}
          strokeDashoffset={circumference(r) * (1 - left)}
          transform="rotate(-90 70 88)"
        />
        {TICKS.map((angle) => (
          <line key={angle} x1="70" y1="28" x2="70" y2={angle % 90 === 0 ? 38 : 34} className="stopwatch-tick" transform={`rotate(${angle} 70 88)`} />
        ))}
        <line x1="70" y1="88" x2="70" y2="32" className="stopwatch-hand" style={{ transform: `rotate(${left * 360}deg)` }} />
        <circle cx="70" cy="88" r="6" className="stopwatch-pin" />
      </svg>
      <span className="timer-readout">
        <span className="timer-seconds">{shown}</span>
        <span className="timer-label">{label}</span>
      </span>
    </>
  );
}

/** A tick every five minutes of the face, as on a watch. */
const TICKS = Array.from({ length: 12 }, (_, i) => i * 30);
