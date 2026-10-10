// The colours the colour picker offers: a palette of shades, a row of
// standard colours, and the colours used since the app started.

import { formatColor, fromHsv } from "../theme/color";
import type { Color } from "../types";
import { createStore } from "../store";

/** The hues of the palette's columns after the greys. */
const HUES = [0, 30, 50, 120, 170, 200, 220, 260, 300];

/** Saturation and value of the palette's rows, from light to dark. */
const TONES: [number, number][] = [
  [0.15, 1],
  [0.35, 1],
  [0.65, 0.95],
  [0.85, 0.75],
  [0.9, 0.5],
];

const GREYS = [1, 0.85, 0.6, 0.35, 0];

/** Rows of shades from light to dark: a grey, then one per hue. */
export const SHADES: Color[][] = TONES.map(([s, v], row) => [
  formatColor(fromHsv({ h: 0, s: 0, v: GREYS[row] })),
  ...HUES.map((h) => formatColor(fromHsv({ h, s, v }))),
]);

export const STANDARD_COLORS: Color[] = [
  "#C00000",
  "#FF0000",
  "#FFC000",
  "#FFFF00",
  "#92D050",
  "#00B050",
  "#00B0F0",
  "#0070C0",
  "#002060",
  "#7030A0",
];

export const RECENT_LIMIT = 10;

/** The colours used since the app started, the latest first. */
export const recentColors = createStore<readonly Color[]>([]);

/** Puts `color` first among the recent colours. */
export function rememberColor(color: Color): void {
  recentColors.set(withRecent(recentColors.get(), color));
}

export function withRecent(recent: readonly Color[], color: Color): readonly Color[] {
  return [color, ...recent.filter((c) => c !== color)].slice(0, RECENT_LIMIT);
}
