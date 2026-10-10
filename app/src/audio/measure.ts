// Measures loudness in a Worker: a long sound takes over a second, and the
// window measuring it is the Director's, which may be running a show.
// Decoding stays in the window, since only a window has OfflineAudioContext.

import type { Span } from "../types";
import { WHOLE_FILE } from "./clip-player";
import { measureLoudness, partRange, type Loudness, type Samples } from "./loudness";

export interface MeasureRequest {
  id: number;
  samples: Samples;
  parts: Span[];
}

export type MeasureReply = { id: number; results: Loudness[] } | { id: number; error: string };

let worker: Worker | null = null;
let nextId = 0;
const waiting = new Map<number, { resolve: (results: Loudness[]) => void; reject: (error: Error) => void }>();

function measurer(): Worker {
  if (worker) return worker;
  const started = new Worker(new URL("./loudness.worker.ts", import.meta.url), { type: "module" });
  started.onmessage = ({ data }: MessageEvent<MeasureReply>) => {
    const request = waiting.get(data.id);
    waiting.delete(data.id);
    if ("error" in data) request?.reject(new Error(data.error));
    else request?.resolve(data.results);
  };
  // A Worker that fails to load or dies fails what it was asked; the next request starts another.
  started.onerror = (event) => {
    event.preventDefault();
    for (const request of waiting.values()) request.reject(new Error(`loudness worker: ${event.message}`));
    waiting.clear();
    started.terminate();
    worker = null;
  };
  worker = started;
  return started;
}

/**
 * The loudness of each of `parts` of `samples`. The samples move to the
 * Worker, so they cannot be used afterwards. Where there is no Worker (the
 * tests) they are measured here.
 */
export function measureParts(samples: Samples, parts: Span[]): Promise<Loudness[]> {
  if (typeof Worker === "undefined") return Promise.resolve(parts.map((part) => measureLoudness(samples, part)));
  return new Promise((resolve, reject) => {
    const id = nextId++;
    waiting.set(id, { resolve, reject });
    const request: MeasureRequest = { id, samples, parts };
    const buffers = new Set(samples.channels.map((channel) => channel.buffer as ArrayBuffer));
    measurer().postMessage(request, [...buffers]);
  });
}

/**
 * The loudness of `part` of `samples`, measured in the Worker on a copy of
 * that part, so `samples` stay usable (a dialog still draws them).
 */
export async function measureCopy(samples: Samples, part: Span): Promise<Loudness> {
  const [first, last] = partRange(samples, part);
  const copy = { sampleRate: samples.sampleRate, channels: samples.channels.map((channel) => channel.slice(first, last)) };
  const [loudness] = await measureParts(copy, [WHOLE_FILE]);
  return loudness;
}
