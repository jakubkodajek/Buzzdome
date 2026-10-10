// The waveform of a sound in the trim dialog: what to draw in each pixel
// column, and which stretch of the sound is in view.

import type { Samples } from "../audio/loudness";

/** The lowest and highest sample of each column, over all channels. */
export interface Peaks {
  min: Float32Array;
  max: Float32Array;
}

/** A stretch of a sound, in ms, that the dialog shows. */
export interface TimeWindow {
  fromMs: number;
  toMs: number;
}

/** The narrowest window zooming goes to. */
export const MIN_WINDOW_MS = 500;

/** Peaks of `window` in `columns` columns. Time O(samples in the window + columns). */
export function waveformPeaks(samples: Samples, window: TimeWindow, columns: number): Peaks {
  const min = new Float32Array(columns);
  const max = new Float32Array(columns);
  const length = samples.channels[0]?.length ?? 0;
  const perMs = samples.sampleRate / 1000;
  const perColumn = ((window.toMs - window.fromMs) * perMs) / columns;
  for (let column = 0; column < columns; column++) {
    const start = Math.min(length, Math.max(0, Math.floor(window.fromMs * perMs + column * perColumn)));
    const end = Math.min(length, Math.max(start + 1, Math.floor(window.fromMs * perMs + (column + 1) * perColumn)));
    let [low, high] = [0, 0];
    for (const channel of samples.channels) {
      for (let i = start; i < end; i++) {
        low = Math.min(low, channel[i]);
        high = Math.max(high, channel[i]);
      }
    }
    min[column] = low;
    max[column] = high;
  }
  return { min, max };
}

/** `window` zoomed by `factor` (below 1 zooms in) around `atMs`, kept inside the sound. */
export function zoomed(window: TimeWindow, atMs: number, factor: number, durationMs: number): TimeWindow {
  const width = Math.min(durationMs, Math.max(MIN_WINDOW_MS, (window.toMs - window.fromMs) * factor));
  const share = window.toMs > window.fromMs ? (atMs - window.fromMs) / (window.toMs - window.fromMs) : 0.5;
  return inside(atMs - share * width, width, durationMs);
}

/** `window` moved by `deltaMs`, kept inside the sound. */
export function panned(window: TimeWindow, deltaMs: number, durationMs: number): TimeWindow {
  return inside(window.fromMs + deltaMs, window.toMs - window.fromMs, durationMs);
}

function inside(fromMs: number, width: number, durationMs: number): TimeWindow {
  const from = Math.min(Math.max(0, durationMs - width), Math.max(0, fromMs));
  return { fromMs: from, toMs: from + width };
}
