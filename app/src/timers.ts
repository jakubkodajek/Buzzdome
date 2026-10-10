// The designs a question's clock is drawn in (quiz_core::TimerDesign).

import type { StringKey } from "./i18n";
import type { TimerDesign } from "./types";

/** The design of a question without one of its own. */
export const DEFAULT_TIMER: TimerDesign = "ring";

/** How each design is named, in the order the editor offers them. */
export const TIMER_LABEL: Record<TimerDesign, StringKey> = {
  ring: "timerRing",
  bar: "timerBar",
  digital: "timerDigital",
  hourglass: "timerHourglass",
  stopwatch: "timerStopwatch",
};

export const TIMER_DESIGNS = Object.keys(TIMER_LABEL) as TimerDesign[];
