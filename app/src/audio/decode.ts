import { readMediaLength } from "../media";
import { LOUDNESS_RATE, type Samples } from "./loudness";

/**
 * The longest sound read whole, for its waveform and its loudness. A minute
 * of stereo takes about 23 MB once decoded, so ten minutes stay near 230 MB
 * in the window that may also run the show.
 */
export const MAX_DECODED_MINUTES = 10;

/**
 * The largest file read whole when it does not tell its length: one that
 * even at 64 kbit/s, a low rate for music, plays no longer than
 * `MAX_DECODED_MINUTES` (about 4.8 MB).
 */
export const MAX_UNTIMED_BYTES = (MAX_DECODED_MINUTES * 60 * 64_000) / 8;

/** A sound longer than `MAX_DECODED_MINUTES`, or one that may be, which is not read whole. */
export class SoundTooLong extends Error {
  constructor(url: string) {
    super(`${url}: longer than ${MAX_DECODED_MINUTES} minutes, or too large to tell`);
    this.name = "SoundTooLong";
  }
}

/**
 * Whether a sound of `lengthMs` is read whole. One that tells no length
 * (NaN) is judged by its size, `bytes` of it known so far.
 */
export function readsWhole(lengthMs: number, bytes: number): boolean {
  return Number.isNaN(lengthMs) ? bytes <= MAX_UNTIMED_BYTES : lengthMs <= MAX_DECODED_MINUTES * 60_000;
}

/**
 * Reads a whole audio file into samples at `LOUDNESS_RATE`, the rate the
 * loudness meter needs; the browser resamples while it decodes (Web Audio
 * API, `decodeAudioData`). A file its metadata says is too long, or that
 * tells no length and is too large, is refused with `SoundTooLong`, as far
 * as can be before it is read.
 */
export async function decodeAudioFile(url: string): Promise<Samples> {
  const lengthMs = await readMediaLength(url, "audio");
  if (!readsWhole(lengthMs, 0)) throw new SoundTooLong(url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  if (!readsWhole(lengthMs, Number(response.headers.get("Content-Length") ?? 0))) throw new SoundTooLong(url);
  const data = await response.arrayBuffer();
  if (!readsWhole(lengthMs, data.byteLength)) throw new SoundTooLong(url);
  const buffer = await new OfflineAudioContext(1, 1, LOUDNESS_RATE).decodeAudioData(data);
  return { sampleRate: buffer.sampleRate, channels: Array.from({ length: buffer.numberOfChannels }, (_, c) => buffer.getChannelData(c)) };
}
