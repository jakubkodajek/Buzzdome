// Plays the part of a sound or video file a quiz asks for (SN-8), a sound at
// a gain that can go above the file's own level (SN-9).

import { MAX_PLAYER_VOLUME } from "../bindings/Constants";
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

/** What the player's volume (percent, see `usePlayerVolume`) multiplies everything the show plays by. */
export function playerLevel(volume: number): number {
  return volume / MAX_PLAYER_VOLUME;
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
 * A sound or video element, played a part at a time: a part plays from its
 * start (or where it was paused) to its end, once or over and over.
 */
export class PartPlayer {
  /** Called when the file starts or stops playing. */
  onChange: (() => void) | null = null;
  private span = WHOLE_FILE;
  private loop = false;
  private ended = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(protected readonly media: HTMLMediaElement) {
    media.addEventListener("play", this.changed);
    media.addEventListener("pause", this.changed);
    media.addEventListener("ended", this.reachedEnd);
  }

  get playing(): boolean {
    return !this.media.paused;
  }

  /** The part played to its end (and does not loop) since it last started. */
  get finished(): boolean {
    return this.ended;
  }

  get positionMs(): number {
    return this.media.currentTime * 1000;
  }

  /** Length of the file; NaN until it is known. */
  get durationMs(): number {
    return this.media.duration * 1000;
  }

  play(span: Span, { loop = false, fromMs }: PlayOptions = {}) {
    const paused = sameSpan(span, this.span) && this.positionMs > span.fromMs && inSpan(span, this.positionMs);
    this.span = span;
    this.loop = loop;
    this.ended = false;
    if (fromMs !== undefined || !paused) this.seek(fromMs ?? span.fromMs);
    this.media.play().catch((e: unknown) => {
      // A pause or a new file before playback starts is no failure.
      if (!(e instanceof DOMException && e.name === "AbortError")) console.error("media:", e);
    });
    this.watchEnd();
  }

  /** Pauses where it is. */
  pause() {
    this.media.pause();
    this.unwatchEnd();
  }

  /** Stops and goes back to the start of the part. */
  stop() {
    this.pause();
    this.seek(this.span.fromMs);
  }

  /** Stops for good; the element stays as it is. */
  dispose() {
    this.pause();
    const media = this.media;
    media.removeEventListener("play", this.changed);
    media.removeEventListener("pause", this.changed);
    media.removeEventListener("ended", this.reachedEnd);
  }

  private seek(ms: number) {
    this.media.currentTime = Math.max(0, ms) / 1000;
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
      this.ended = true;
    }
  };
}

/**
 * One audio file, played a part at a time. A gain above 1 (louder than the
 * file) goes through the window's audio context, which needs the file's
 * server to allow it; Tauri's asset protocol does (CORS header).
 */
export class ClipPlayer extends PartPlayer {
  private readonly source: MediaElementAudioSourceNode | null = null;
  private readonly amplifier: GainNode | null = null;

  /** `gain` is the most it plays at: above 1 only when made with a gain above 1. */
  constructor(url: string, gain = 1) {
    const audio = new Audio();
    super(audio);
    audio.preload = "auto";
    if (gain > 1) audio.crossOrigin = "anonymous";
    audio.src = url;
    if (gain > 1) {
      const context = audioContext();
      this.source = context.createMediaElementSource(audio);
      this.amplifier = new GainNode(context, { gain });
      this.source.connect(this.amplifier).connect(context.destination);
    }
    this.setGain(gain);
  }

  /** Plays at `gain` from now on, also while it plays. */
  setGain(gain: number) {
    if (this.amplifier) this.amplifier.gain.value = Math.max(0, gain);
    else this.media.volume = Math.min(1, Math.max(0, gain));
  }

  /** Stops for good and lets go of the file. */
  override dispose() {
    super.dispose();
    this.media.removeAttribute("src");
    this.media.load();
    this.source?.disconnect();
  }
}
