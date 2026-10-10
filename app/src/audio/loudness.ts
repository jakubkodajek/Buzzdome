// How loud a sound is, measured as ITU-R BS.1770-4 does (K-weighted, gated
// integrated loudness in LUFS), and the gain that brings it to the quiz's
// loudness (SN-9). https://www.itu.int/rec/R-REC-BS.1770

import { MAX_GAIN_DB, MIN_GAIN_DB } from "../bindings/Constants";
import type { Span } from "../types";
import { WHOLE_FILE } from "./clip-player";

/** Sample rate of the samples measured; the filters below are for it. */
export const LOUDNESS_RATE = 48_000;

/** Targets to choose from: Standard, Quieter, Broadcast (EBU R 128, https://tech.ebu.ch/publications/r128). */
export const LOUDNESS_TARGETS = [-16, -18, -23] as const;
export type LoudnessTarget = (typeof LOUDNESS_TARGETS)[number];

/** A sound is raised only as far as its peaks stay at or below this, in dBFS. */
export const PEAK_CEILING_DB = -1;

/** Within this many LU of the target a sound needs no gain (our choice, below the 1 LU EBU R 128 allows live). */
const AT_TARGET_LU = 0.5;

/** Gating blocks (BS.1770-4, 2.8): 400 ms long, a new one every 100 ms. */
const BLOCK_MS = 400;
const STEP_MS = 100;
const STEPS_PER_BLOCK = BLOCK_MS / STEP_MS;
const ABSOLUTE_GATE_LUFS = -70;
const RELATIVE_GATE_LU = -10;

/** One biquad filter stage: y = b0 x + b1 x[-1] + b2 x[-2] - a1 y[-1] - a2 y[-2]. */
interface Biquad {
  b0: number;
  b1: number;
  b2: number;
  a1: number;
  a2: number;
}

/** K-weighting at 48 kHz: the head's shelf (BS.1770-4 Table 1), then the RLB high-pass (Table 2). */
const K_WEIGHTING: readonly Biquad[] = [
  { b0: 1.53512485958697, b1: -2.69169618940638, b2: 1.19839281085285, a1: -1.69065929318241, a2: 0.73248077421585 },
  { b0: 1.0, b1: -2.0, b2: 1.0, a1: -1.99004745483398, a2: 0.99007225036621 },
];

/** A decoded sound: one array of samples per channel, in Web Audio's channel order. */
export interface Samples {
  sampleRate: number;
  channels: Float32Array[];
}

export interface Loudness {
  /** Integrated loudness in LUFS; null when nothing passes the gates or the sound is shorter than a block. */
  lufs: number | null;
  /** Highest sample, in dBFS; -Infinity for silence. */
  peakDb: number;
  /** Length of what was measured. */
  durationMs: number;
}

/**
 * How much each channel counts (BS.1770-4 Table 3): left, right and centre
 * 1.0, surround 1.41, the low-frequency channel not at all. Web Audio
 * orders 5.1 sound L, R, C, LFE, SL, SR; other layouts count every channel
 * fully.
 */
export function channelWeights(count: number): number[] {
  return count === 6 ? [1, 1, 1, 0, 1.41, 1.41] : Array<number>(count).fill(1);
}

/** The samples `part` covers: the first and the one after the last. */
export function partRange(samples: Samples, part: Span): [number, number] {
  const length = samples.channels[0]?.length ?? 0;
  const at = (ms: number) => Math.min(length, Math.max(0, Math.round((ms * samples.sampleRate) / 1000)));
  const first = at(part.fromMs);
  return [first, part.toMs === null ? length : Math.max(first, at(part.toMs))];
}

/** Measures `part` of `samples`, which must be at `LOUDNESS_RATE`. Time O(samples), memory O(samples / 4800). */
export function measureLoudness(samples: Samples, part: Span = WHOLE_FILE): Loudness {
  const { sampleRate, channels } = samples;
  if (sampleRate !== LOUDNESS_RATE) throw new Error(`loudness is measured at ${LOUDNESS_RATE} Hz, not ${sampleRate} Hz`);
  const [first, last] = partRange(samples, part);
  const step = (sampleRate * STEP_MS) / 1000;
  const steps = Math.floor((last - first) / step);

  // Weighted energy of every 100 ms step, summed over the channels.
  const energy = new Float64Array(steps);
  let peak = 0;
  const weights = channelWeights(channels.length);
  channels.forEach((channel, c) => {
    const x = channel.subarray(first, last);
    for (let i = 0; i < x.length; i++) peak = Math.max(peak, Math.abs(x[i]));
    if (weights[c] === 0) return;
    const y = kWeighted(x.subarray(0, steps * step));
    for (let k = 0; k < steps; k++) {
      let sum = 0;
      for (let i = k * step; i < (k + 1) * step; i++) sum += y[i] * y[i];
      energy[k] += weights[c] * sum;
    }
  });

  const blocks: number[] = [];
  for (let j = 0; j + STEPS_PER_BLOCK <= steps; j++) {
    let sum = 0;
    for (let k = j; k < j + STEPS_PER_BLOCK; k++) sum += energy[k];
    blocks.push(sum / (STEPS_PER_BLOCK * step));
  }
  const loud = blocks.filter((z) => lufsOf(z) > ABSOLUTE_GATE_LUFS);
  const relativeGate = lufsOf(mean(loud)) + RELATIVE_GATE_LU;
  const gated = loud.filter((z) => lufsOf(z) > relativeGate);
  return {
    lufs: gated.length > 0 ? lufsOf(mean(gated)) : null,
    peakDb: 20 * Math.log10(peak),
    durationMs: ((last - first) * 1000) / sampleRate,
  };
}

/** Loudness of a mean square (BS.1770-4 equation 2). */
function lufsOf(meanSquare: number): number {
  return -0.691 + 10 * Math.log10(meanSquare);
}

function mean(values: number[]): number {
  return values.reduce((sum, v) => sum + v, 0) / values.length;
}

/** `x` through the K-weighting stages, as a new array. */
function kWeighted(x: Float32Array): Float64Array {
  const y = Float64Array.from(x);
  for (const { b0, b1, b2, a1, a2 } of K_WEIGHTING) {
    let x1 = 0;
    let x2 = 0;
    let y1 = 0;
    let y2 = 0;
    for (let i = 0; i < y.length; i++) {
      const input = y[i];
      const output = b0 * input + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
      x2 = x1;
      x1 = input;
      y2 = y1;
      y1 = output;
      y[i] = output;
    }
  }
  return y;
}

export type LoudnessStatus =
  /** The gain brings the sound to the target. */
  | "changed"
  /** Already within `AT_TARGET_LU`: no gain. */
  | "atTarget"
  /** Raised less than the target asks, to keep peaks at `PEAK_CEILING_DB`, or kept within the gain limits. */
  | "limited"
  | "silent"
  /** Shorter than one 400 ms block, which BS.1770 cannot measure. */
  | "tooShort";

export interface Normalization {
  status: LoudnessStatus;
  /** The sound's new loudness gain in dB, to 0.1 dB. */
  gainDb: number;
}

/**
 * The loudness gain that brings a sound measured as `loudness` to `target`
 * LUFS. A sound is raised only as far as its peaks stay at or below
 * `PEAK_CEILING_DB`; it is never lowered for its peaks alone, since the file
 * itself plays them unchanged.
 */
export function normalization(loudness: Loudness, target: number): Normalization {
  if (loudness.lufs === null) return { status: loudness.durationMs < BLOCK_MS ? "tooShort" : "silent", gainDb: 0 };
  const wanted = target - loudness.lufs;
  if (Math.abs(wanted) < AT_TARGET_LU) return { status: "atTarget", gainDb: 0 };
  const raised = wanted > 0 ? Math.min(wanted, Math.max(0, PEAK_CEILING_DB - loudness.peakDb)) : wanted;
  const gainDb = tenth(Math.min(MAX_GAIN_DB, Math.max(MIN_GAIN_DB, raised)));
  return { status: gainDb === tenth(wanted) ? "changed" : "limited", gainDb };
}

function tenth(value: number): number {
  return Math.round(value * 10) / 10;
}
