// Plays the show's sounds (spec SN-1 to SN-4): effects once when their
// moment comes, and music over and over while the show is at its moment,
// fading from one piece to the next and away while other sound plays. The
// app's own sounds are read whole and play to the sample, so its music
// loops without a seam; a quiz's own files may be long, so they stream.

import { ClipPlayer, WHOLE_FILE } from "./clip-player";
import { audioContext } from "./context";
import type { SoundSource } from "./library";

/** How long music takes to fade in, in seconds. */
const FADE_IN_S = 0.4;
/** How long music takes to fade out when the show's moment changes. */
export const FADE_OUT_S = 1.2;
/** How long music takes to go quiet under other sound, and to come back. */
const DUCK_S = 0.3;
/** How long a sound cut short takes to fade, so that it does not click. */
export const CUT_S = 0.05;
/** How smoothly a volume follows a change. */
const VOLUME_S = 0.05;
/**
 * How far ahead the next round of a loop is set to start, in seconds:
 * timers in a busy window run late, the audio clock does not.
 */
const SCHEDULE_AHEAD_S = 2;
const SCHEDULE_EVERY_MS = 500;
/** The app's sounds kept read: the show's moments take at most 15, and a few more for trying others. */
const MAX_READ = 20;

/** Something that plays, until it ends or is stopped. */
export interface Playing {
  stop(): void;
}

/** Plays a quiz's sounds into the speakers, at the volumes it is given. */
export class ShowAudio {
  private readonly effectsOut: GainNode;
  private readonly musicOut: GainNode;
  /** Takes the music away while other sound plays. */
  private readonly ducking: GainNode;
  /** The quiz's own files that effects play, open to start at once, by URL. */
  private readonly streams = new Map<string, Stream>();
  private bed: { url: string; music: Music } | null = null;

  constructor() {
    const context = this.context;
    this.effectsOut = new GainNode(context);
    this.effectsOut.connect(context.destination);
    this.musicOut = new GainNode(context);
    this.ducking = new GainNode(context);
    this.ducking.connect(this.musicOut).connect(context.destination);
  }

  /** How loud the effects and the music play: what their samples are multiplied by. */
  setVolumes(effects: number, music: number) {
    fadeTo(this.effectsOut, effects, VOLUME_S);
    fadeTo(this.musicOut, music, VOLUME_S);
  }

  /**
   * Gets ready to play `effects` and `music` at once: reads the app's
   * sounds among them, and opens the quiz's files the effects play. Files
   * no longer among the effects are let go.
   */
  prepare(effects: readonly SoundSource[], music: readonly SoundSource[]) {
    for (const source of [...effects, ...music]) if (source.builtIn) read(source.url).catch(failed(source.url));
    const files = new Set(effects.filter((source) => !source.builtIn).map((source) => source.url));
    for (const [url, stream] of this.streams) {
      if (files.has(url)) continue;
      stream.dispose();
      this.streams.delete(url);
    }
    for (const url of files) if (!this.streams.has(url)) this.streams.set(url, new Stream(url, this.effectsOut));
  }

  /** Plays `source` once, from its start; `onEnd` is told when it has played. */
  effect(source: SoundSource, onEnd?: () => void): Playing {
    return this.playOnce(source, 0, false, onEnd);
  }

  /**
   * Plays `source` from `fromMs` into it as of now: should it start later
   * (the file still being read), it starts that much further in, so that it
   * keeps to the clock it counts down.
   */
  inTime(source: SoundSource, fromMs: number): Playing {
    return this.playOnce(source, fromMs, true);
  }

  /**
   * Plays `source` over and over from now on, fading out what played; null
   * plays no music. The music that plays already goes on.
   */
  music(source: SoundSource | null) {
    if ((source?.url ?? null) === (this.bed?.url ?? null)) return;
    this.bed?.music.stop(FADE_OUT_S);
    this.bed = source && { url: source.url, music: source.builtIn ? new ReadLoop(this.context, source, this.ducking) : new StreamLoop(this.context, source.url, this.ducking) };
  }

  /** Takes the music away while `on`, and brings it back after. */
  duck(on: boolean) {
    fadeTo(this.ducking, on ? 0 : 1, DUCK_S);
  }

  /** The window's audio context, asked to run: one made before the quizmaster clicked anything starts suspended. */
  private get context(): AudioContext {
    return audioContext();
  }

  /** Stops everything for good. */
  dispose() {
    this.music(null);
    for (const stream of this.streams.values()) stream.dispose();
    this.streams.clear();
    // The music fades out first.
    setTimeout(() => {
      this.effectsOut.disconnect();
      this.musicOut.disconnect();
    }, FADE_OUT_S * 1000);
  }

  private playOnce(source: SoundSource, fromMs: number, keepTime: boolean, onEnd?: () => void): Playing {
    if (!source.builtIn) {
      const stream = this.streams.get(source.url);
      if (stream) return stream.play(fromMs, onEnd);
      // Not readied (a sound tried out in the editor): opened now.
      const once = new ClipPlayer(source.url, 1, this.effectsOut);
      once.onEnd = () => {
        once.dispose();
        onEnd?.();
      };
      once.play(WHOLE_FILE, { fromMs });
      return { stop: () => once.dispose() };
    }
    const asked = this.context.currentTime;
    let stopped = false;
    let playing: Round | null = null;
    read(source.url).then((buffer) => {
      if (stopped) return;
      const late = keepTime ? this.context.currentTime - asked : 0;
      const offset = fromMs / 1000 + late;
      if (offset >= buffer.duration) return;
      playing = startRound(this.context, buffer, this.effectsOut, this.context.currentTime, offset);
      playing.source.addEventListener("ended", () => onEnd?.());
    }, failed(source.url));
    return {
      stop: () => {
        stopped = true;
        if (playing) stopRound(playing, CUT_S);
      },
    };
  }
}

/** The starts of a loop's rounds due before `until`, the next round starting at `next`, `loopS` apart. */
export function roundsDue(next: number, until: number, loopS: number): { starts: number[]; next: number } {
  const starts: number[] = [];
  for (; next < until; next += loopS) starts.push(next);
  return { starts, next };
}

/** Music that fades in as it starts, and out when it stops. */
interface Music {
  stop(fadeS: number): void;
}

/**
 * One of the app's pieces of music: each round starts `loopMs` after the
 * one before, while the rest of the file rings on under it.
 */
class ReadLoop implements Music {
  private readonly level: GainNode;
  private readonly rounds = new Set<Round>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private stopped = false;

  constructor(
    private readonly context: AudioContext,
    source: SoundSource,
    output: AudioNode,
  ) {
    this.level = new GainNode(context, { gain: 0 });
    this.level.connect(output);
    read(source.url).then((buffer) => {
      if (this.stopped) return;
      const loopS = source.loopMs !== null && source.loopMs > 0 ? source.loopMs / 1000 : buffer.duration;
      let next = context.currentTime;
      const schedule = () => {
        const due = roundsDue(next, context.currentTime + SCHEDULE_AHEAD_S, loopS);
        for (const at of due.starts) this.startRound(buffer, at);
        next = due.next;
      };
      schedule();
      this.timer = setInterval(schedule, SCHEDULE_EVERY_MS);
      fadeTo(this.level, 1, FADE_IN_S);
    }, failed(source.url));
  }

  stop(fadeS: number) {
    this.stopped = true;
    if (this.timer !== null) clearInterval(this.timer);
    fadeTo(this.level, 0, fadeS);
    const end = this.context.currentTime + fadeS;
    // One set to start later never starts.
    for (const round of this.rounds) round.source.stop(end);
    setTimeout(() => this.level.disconnect(), fadeS * 1000);
  }

  private startRound(buffer: AudioBuffer, at: number) {
    const round = startRound(this.context, buffer, this.level, at, 0);
    this.rounds.add(round);
    round.source.addEventListener("ended", () => this.rounds.delete(round));
  }
}

/** A quiz's own music file, streamed, over and over. */
class StreamLoop implements Music {
  private readonly level: GainNode;
  private readonly player: ClipPlayer;

  constructor(context: AudioContext, url: string, output: AudioNode) {
    this.level = new GainNode(context, { gain: 0 });
    this.level.connect(output);
    this.player = new ClipPlayer(url, 1, this.level);
    this.player.onEnd = () => console.error(`music ${url}: cannot be played`);
    this.player.play(WHOLE_FILE, { loop: true });
    fadeTo(this.level, 1, FADE_IN_S);
  }

  stop(fadeS: number) {
    fadeTo(this.level, 0, fadeS);
    setTimeout(() => {
      this.player.dispose();
      this.level.disconnect();
    }, fadeS * 1000);
  }
}

/** A quiz's own file an effect plays, open, so that it starts at once. */
class Stream {
  private readonly player: ClipPlayer;
  /** Plays so far; a stop for one before the last one does nothing. */
  private plays = 0;
  private ended: (() => void) | undefined;

  constructor(url: string, output: AudioNode) {
    this.player = new ClipPlayer(url, 1, output);
    this.player.onEnd = () => this.ended?.();
  }

  play(fromMs: number, onEnd?: () => void): Playing {
    const play = ++this.plays;
    this.ended = onEnd;
    this.player.play(WHOLE_FILE, { fromMs });
    return { stop: () => play === this.plays && this.player.stop() };
  }

  dispose() {
    this.player.dispose();
  }
}

/** A sound read whole, playing through a level of its own, so that it can fade. */
interface Round {
  source: AudioBufferSourceNode;
  level: GainNode;
}

/** Starts `buffer` at `at` on the audio clock, `offset` seconds into it. */
function startRound(context: AudioContext, buffer: AudioBuffer, output: AudioNode, at: number, offset: number): Round {
  const level = new GainNode(context);
  level.connect(output);
  const source = new AudioBufferSourceNode(context, { buffer });
  source.connect(level);
  source.addEventListener("ended", () => level.disconnect());
  source.start(at, offset);
  return { source, level };
}

function stopRound(round: Round, fadeS: number) {
  fadeTo(round.level, 0, fadeS);
  round.source.stop(round.level.context.currentTime + fadeS);
}

/** Takes `node`'s gain from where it is to `value` in about `seconds`. */
function fadeTo(node: GainNode, value: number, seconds: number) {
  const now = node.context.currentTime;
  node.gain.cancelScheduledValues(now);
  // Five time constants come within 1 % of the value.
  node.gain.setTargetAtTime(value, now, seconds / 5);
}

const reads = new Map<string, Promise<AudioBuffer>>();

/** One of the app's sounds, read whole; the last ones read stay read. */
function read(url: string): Promise<AudioBuffer> {
  const known = reads.get(url);
  if (known) {
    // Read again lately: the last to be let go.
    reads.delete(url);
    reads.set(url, known);
    return known;
  }
  const reading = fetch(url)
    .then((response) => {
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return response.arrayBuffer();
    })
    .then((data) => audioContext().decodeAudioData(data));
  reads.set(url, reading);
  // One that failed is read again when next asked for.
  reading.catch(() => reads.get(url) === reading && reads.delete(url));
  for (const old of reads.keys()) {
    if (reads.size <= MAX_READ) break;
    reads.delete(old);
  }
  return reading;
}

function failed(url: string) {
  return (e: unknown) => console.error(`sound ${url}:`, e);
}
