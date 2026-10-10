import { readMediaLength } from "../media";
import { LOUDNESS_RATE, type Samples } from "./loudness";

/**
 * The longest sound read whole, for its waveform and its loudness. A minute
 * of stereo takes about 23 MB once decoded, so ten minutes stay near 230 MB
 * in the window that may also run the show.
 */
export const MAX_DECODED_MINUTES = 10;

/** A sound longer than `MAX_DECODED_MINUTES`, which is not read whole. */
export class SoundTooLong extends Error {
  constructor(url: string) {
    super(`${url}: longer than ${MAX_DECODED_MINUTES} minutes`);
    this.name = "SoundTooLong";
  }
}

/** Whether a sound of `lengthMs` is read whole; one that tells no length (NaN) is. */
export function readsWhole(lengthMs: number): boolean {
  return !(lengthMs > MAX_DECODED_MINUTES * 60_000);
}

/**
 * Reads a whole audio file into samples at `LOUDNESS_RATE`, the rate the
 * loudness meter needs; the browser resamples while it decodes (Web Audio
 * API, `decodeAudioData`). A file its metadata says is too long is refused
 * with `SoundTooLong` before it is read.
 */
export async function decodeAudioFile(url: string): Promise<Samples> {
  if (!readsWhole(await readMediaLength(url, "audio"))) throw new SoundTooLong(url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  const buffer = await new OfflineAudioContext(1, 1, LOUDNESS_RATE).decodeAudioData(await response.arrayBuffer());
  return { sampleRate: buffer.sampleRate, channels: Array.from({ length: buffer.numberOfChannels }, (_, c) => buffer.getChannelData(c)) };
}
