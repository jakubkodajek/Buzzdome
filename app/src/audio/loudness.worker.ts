// The Worker that `measure.ts` hands decoded sounds to.

import { measureLoudness } from "./loudness";
import type { MeasureReply, MeasureRequest } from "./measure";

self.onmessage = ({ data }: MessageEvent<MeasureRequest>) => {
  let reply: MeasureReply;
  try {
    reply = { id: data.id, results: data.parts.map((part) => measureLoudness(data.samples, part)) };
  } catch (error) {
    reply = { id: data.id, error: String(error) };
  }
  self.postMessage(reply);
};
