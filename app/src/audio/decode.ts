import { LOUDNESS_RATE, type Samples } from "./loudness";

/**
 * Reads a whole audio file into samples at `LOUDNESS_RATE`, the rate the
 * loudness meter needs; the browser resamples while it decodes (Web Audio
 * API, `decodeAudioData`). Five minutes of stereo take about 115 MB.
 */
export async function decodeAudioFile(url: string): Promise<Samples> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  const buffer = await new OfflineAudioContext(1, 1, LOUDNESS_RATE).decodeAudioData(await response.arrayBuffer());
  return { sampleRate: buffer.sampleRate, channels: Array.from({ length: buffer.numberOfChannels }, (_, c) => buffer.getChannelData(c)) };
}
