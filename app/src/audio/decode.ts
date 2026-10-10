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

/**
 * The largest file read whole when it tells its length: `MAX_DECODED_MINUTES`
 * of CD-quality WAV (1,411.2 kbit/s, about 106 MB). A high-resolution WAV of
 * the same length is three times that before it is even decoded.
 */
export const MAX_TIMED_BYTES = (MAX_DECODED_MINUTES * 60 * 1_411_200) / 8;

/**
 * How long reading a sound's length may take before it is read whole. The
 * quizmaster waits for the dialog anyway, and a file that does not tell in
 * time fails to read rather than being judged by its size as one that tells
 * none: a long file on a slow USB drive would be refused as too long (Z8).
 */
const LENGTH_TIMEOUT_MS = 60_000;

/** A sound longer than `MAX_DECODED_MINUTES`, or a file too large, which is not read whole. */
export class SoundTooLong extends Error {
  constructor(url: string) {
    super(`${url}: longer than ${MAX_DECODED_MINUTES} minutes, or too large`);
    this.name = "SoundTooLong";
  }
}

/** The most of a sound of `lengthMs` that is read; one that tells no length (NaN) is read only while it is small. */
export function maxBytes(lengthMs: number): number {
  return Number.isNaN(lengthMs) ? MAX_UNTIMED_BYTES : MAX_TIMED_BYTES;
}

/** Whether a sound of `lengthMs` is read whole, `bytes` of it known so far. */
export function readsWhole(lengthMs: number, bytes: number): boolean {
  return !(lengthMs > MAX_DECODED_MINUTES * 60_000) && bytes <= maxBytes(lengthMs);
}

/** The body of `response`, or null once it passes `maxBytes`: then the rest is not read. */
export async function readAtMost(response: Response, maxBytes: number): Promise<ArrayBuffer | null> {
  const reader = response.body?.getReader();
  if (!reader) return new ArrayBuffer(0);
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    bytes += value.byteLength;
    if (bytes > maxBytes) {
      await reader.cancel();
      return null;
    }
    chunks.push(value);
  }
  const data = new Uint8Array(bytes);
  let at = 0;
  for (const chunk of chunks) {
    data.set(chunk, at);
    at += chunk.byteLength;
  }
  return data.buffer;
}

/**
 * Reads a whole audio file into samples at `LOUDNESS_RATE`, the rate the
 * loudness meter needs; the browser resamples while it decodes (Web Audio
 * API, `decodeAudioData`). A file its metadata says is too long, or that is
 * larger than `maxBytes`, is refused with `SoundTooLong`, before it is read
 * or as soon as it passes the size; one that does not tell its length in
 * time fails with `MediaLengthTimeout`.
 */
export async function decodeAudioFile(url: string): Promise<Samples> {
  const lengthMs = await readMediaLength(url, "audio", LENGTH_TIMEOUT_MS);
  if (!readsWhole(lengthMs, 0)) throw new SoundTooLong(url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  if (!readsWhole(lengthMs, Number(response.headers.get("Content-Length") ?? 0))) throw new SoundTooLong(url);
  // Read only up to the size, whether the size is told or not.
  const data = await readAtMost(response, maxBytes(lengthMs));
  if (!data) throw new SoundTooLong(url);
  const buffer = await new OfflineAudioContext(1, 1, LOUDNESS_RATE).decodeAudioData(data);
  return { sampleRate: buffer.sampleRate, channels: Array.from({ length: buffer.numberOfChannels }, (_, c) => buffer.getChannelData(c)) };
}
