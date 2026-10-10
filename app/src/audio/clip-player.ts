// Plays the part of an audio file a quiz asks for (SN-8), at a gain that can
// go above the file's own level (SN-9).

import type { Span } from "../types";
import { audioContext } from "./context";

/** How often a playing part checks whether it reached its end, in ms. */
const END_CHECK_MS = 20;

/** All of a file. */
export const WHOLE_FILE: Span = { fromMs: 0, toMs: null };

/** What a sound's samples are multiplied by: its volume (0-100 %) and its loudness gain (dB). */
export function linearGain(volume: number, gainDb: number): number {
  return (volume / 100) * 10 ** (gainDb / 20);
}

/** Whether `ms` lies in `span`; its end is not. */
export function inSpan(span: Span, ms: number): boolean {
  return ms >= span.fromMs && (span.toMs === null || ms < span.toMs);
}

export function sameSpan(a: Span, b: Span): boolean {
  return a.fromMs === b.fromMs && a.toMs === b.toMs;
}

export interface PlayOptions {
  /** Start the part again each time it ends. */
  loop?: boolean;
  /** Where to start; by default where the part was paused, or its start. */
  fromMs?: number;
}

/**
 * One audio file, played a part at a time. A gain above 1 (louder than the
 * file) goes through the window's audio context, which needs the file's
 * server to allow it; Tauri's asset protocol does (CORS header).
 */
export class ClipPlayer {
  /** Called when the file starts or stops playing. */
  onChange: (() => void) | null = null;
  private readonly audio = new Audio();
  private readonly source: MediaElementAudioSourceNode | null = null;
  private span = WHOLE_FILE;
  private loop = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(url: string, gain = 1) {
    const audio = this.audio;
    audio.preload = "auto";
    if (gain > 1) audio.crossOrigin = "anonymous";
    audio.src = url;
    if (gain > 1) {
      const context = audioContext();
      this.source = context.createMediaElementSource(audio);
      this.source.connect(new GainNode(context, { gain })).connect(context.destination);
    } else {
      audio.volume = Math.max(0, gain);
    }
    audio.addEventListener("play", this.changed);
    audio.addEventListener("pause", this.changed);
    audio.addEventListener("ended", this.reachedEnd);
  }

  get playing(): boolean {
    return !this.audio.paused;
  }

  get positionMs(): number {
    return this.audio.currentTime * 1000;
  }

  /** Length of the file; NaN until it is known. */
  get durationMs(): number {
    return this.audio.duration * 1000;
  }

  play(span: Span, { loop = false, fromMs }: PlayOptions = {}) {
    const paused = sameSpan(span, this.span) && this.positionMs > span.fromMs && inSpan(span, this.positionMs);
    this.span = span;
    this.loop = loop;
    if (fromMs !== undefined || !paused) this.seek(fromMs ?? span.fromMs);
    this.audio.play().catch((e: unknown) => console.error("sound:", e));
    this.watchEnd();
  }

  /** Pauses where it is. */
  pause() {
    this.audio.pause();
    this.unwatchEnd();
  }

  /** Stops and goes back to the start of the part. */
  stop() {
    this.pause();
    this.seek(this.span.fromMs);
  }

  /** Stops for good and lets go of the file. */
  dispose() {
    this.pause();
    const audio = this.audio;
    audio.removeEventListener("play", this.changed);
    audio.removeEventListener("pause", this.changed);
    audio.removeEventListener("ended", this.reachedEnd);
    audio.removeAttribute("src");
    audio.load();
    this.source?.disconnect();
  }

  private seek(ms: number) {
    this.audio.currentTime = Math.max(0, ms) / 1000;
  }

  private watchEnd() {
    this.unwatchEnd();
    const end = this.span.toMs;
    // The end of the file needs no watching: `ended` reports it.
    if (end !== null) this.timer = setInterval(() => this.positionMs >= end && this.reachedEnd(), END_CHECK_MS);
  }

  private unwatchEnd() {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
  }

  private readonly changed = () => this.onChange?.();

  private readonly reachedEnd = () => {
    // A part that starts past the end of the file would loop forever.
    if (this.loop && !(this.span.fromMs >= this.durationMs)) {
      this.play(this.span, { loop: true, fromMs: this.span.fromMs });
    } else {
      this.stop();
    }
  };
}
