// Reads a question out in the quiz's language (spec SN-10): the clip the
// app's offline voice rendered for it beforehand (app/src-tauri/src/voice.rs),
// or, while there is none, a voice of the system (the window's Web Speech
// API). The Director prepares every question's clip once a show with
// reading on starts, so the show plays them without waiting.

import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { ANSWER_LETTERS } from "../answers";
import { DEFAULT_QUIZ_SOUNDS, MAX_VOICE } from "../bindings/Constants";
import { useLatest, usePlayerVolume } from "../hooks";
import { settingsOf, soundsOf } from "../quiz";
import { createStore, useStore } from "../store";
import type { Translate } from "../i18n";
import type { GameView, Question, Quiz, QuizLanguage, QuizSettings, ReadAloud, Slide, VoiceLine, VoiceModel } from "../types";
import { ClipPlayer, linearGain, playerLevel } from "./clip-player";
import { playBundled, type Playing } from "./show-audio";

/** The language tag a system voice of each quiz language has, region included. */
export const SPEECH_TAGS: Record<QuizLanguage, string> = {
  cs: "cs-CZ",
  en: "en-GB",
};

/** Every voice of the model, by number. */
export const VOICES: readonly number[] = Array.from({ length: MAX_VOICE + 1 }, (_, voice) => voice);
/** Voices from this number on sound male; the ones before, female [no source - by the pitch of the samples]. */
const FIRST_MALE_VOICE = 5;

/** A voice as the quizmaster sees it: "Female voice 2". */
export function voiceName(t: Translate, voice: number): string {
  return voice < FIRST_MALE_VOICE ? t("voiceFemale", { n: voice + 1 }) : t("voiceMale", { n: voice - FIRST_MALE_VOICE + 1 });
}

const SAMPLES = import.meta.glob<string>("./voices/*.mp3", {
  eager: true,
  query: "?url",
  import: "default",
});

/** A short sample of `voice` speaking `language`, shipped with the app (`voices/README.md`). */
export function sampleUrl(language: QuizLanguage, voice: number): string | null {
  return SAMPLES[`./voices/${language}-${voice}.mp3`] ?? null;
}

/** The words read for `question`: its text, and with the answers each after its letter. */
export function spokenText(question: Pick<Question, "text" | "answers">, what: ReadAloud): string {
  if (what === "off") return "";
  const parts = [question.text];
  if (what === "questionAndAnswers") {
    question.answers.forEach((answer, i) => {
      if (answer.trim() !== "") parts.push(`${ANSWER_LETTERS[i]}: ${answer}`);
    });
  }
  return parts
    .map(sentence)
    .filter((part) => part !== "")
    .join(" ");
}

/** `text` on one line, ending as a sentence ends, so the voice pauses after it. */
function sentence(text: string): string {
  const words = text.replace(/\s+/g, " ").trim();
  return words === "" || /[.!?…:;]$/.test(words) ? words : `${words}.`;
}

/** What the voice says for `slide` by the quiz's `settings` (reading as `what`); null for nothing. */
export function voiceLine(slide: Slide | null | undefined, settings: QuizSettings, what: ReadAloud = settings.readAloud): VoiceLine | null {
  if (slide?.type !== "question") return null;
  const text = spokenText(slide, what);
  return text === "" ? null : { text, language: settings.language, voice: settings.voice };
}

/** Every line the quiz's questions are read with, each once, in slide order. */
export function questionLines(quiz: Quiz): VoiceLine[] {
  const settings = settingsOf(quiz);
  const seen = new Set<string>();
  return quiz.slides.flatMap((slide) => {
    const line = voiceLine(slide, settings);
    if (!line || seen.has(line.text)) return [];
    seen.add(line.text);
    return [line];
  });
}

/** The parts of a system voice the choice looks at. */
export interface SystemVoice {
  name: string;
  lang: string;
  localService: boolean;
  default: boolean;
}

/**
 * The system voice for `language`: one that runs on this computer before
 * one that needs the internet, then one for the language's own region,
 * then the system's default. Null when none speaks it.
 */
export function pickSystemVoice<V extends SystemVoice>(voices: readonly V[], language: QuizLanguage): V | null {
  const tag = SPEECH_TAGS[language].toLowerCase();
  const rank = (voice: V): number | null => {
    const lang = voice.lang.replace("_", "-").toLowerCase();
    const fit = lang === tag ? 0 : lang.split("-")[0] === language ? 1 : null;
    if (fit === null) return null;
    return (voice.localService ? 0 : 4) + fit * 2 + (voice.default ? 0 : 1);
  };
  let best: V | null = null;
  let bestRank = Infinity;
  for (const voice of voices) {
    const r = rank(voice);
    if (r !== null && r < bestRank) [best, bestRank] = [voice, r];
  }
  return best;
}

function speech(): SpeechSynthesis | null {
  return typeof window !== "undefined" && "speechSynthesis" in window ? window.speechSynthesis : null;
}

/** The system's voices, as they load (Chromium lists them only after `voiceschanged`). */
const systemVoices = createStore<readonly SpeechSynthesisVoice[]>([]);
let watchingVoices = false;

function watchVoices() {
  const synth = speech();
  if (watchingVoices || !synth) return;
  watchingVoices = true;
  const load = () => systemVoices.set(synth.getVoices());
  load();
  synth.addEventListener("voiceschanged", load);
}

/** The system voice for `language`; undefined where the system cannot speak at all. */
export function useSystemVoice(language: QuizLanguage): SpeechSynthesisVoice | null | undefined {
  useEffect(watchVoices, []);
  const voices = useStore(systemVoices);
  return useMemo(() => (speech() ? pickSystemVoice(voices, language) : undefined), [voices, language]);
}

/** Whether the voice model is downloaded: null until known. Shared, so a download in the settings shows in the Director. */
const voiceModel = createStore<VoiceModel | null>(null);
let askedModel = false;

export function setVoiceModel(model: VoiceModel) {
  voiceModel.set(model);
}

export function useVoiceModel(): VoiceModel | null {
  useEffect(() => {
    if (askedModel) return;
    askedModel = true;
    api.voiceModel().then(setVoiceModel, () => {
      askedModel = false;
    });
  }, []);
  return useStore(voiceModel);
}

/** How far the clips of the running show's questions are rendered. */
export type Preparing =
  | { state: "off" }
  | { state: "noModel" }
  | { state: "rendering"; done: number; total: number }
  | { state: "ready"; total: number }
  /** Done, but `failed` of the clips could not be rendered: the system voice reads those. */
  | { state: "failed"; failed: number; total: number };

const preparing = createStore<Preparing>({ state: "off" });
/** How often preparing looks again whether the question being asked is over. */
const WAIT_WHILE_ASKING_MS = 500;

export function usePreparing(): Preparing {
  return useStore(preparing);
}

/**
 * Renders the clip of each of `quiz`'s questions that is not kept yet,
 * one after another, while the show with `quiz` runs. A clip that cannot
 * be rendered is read by the system voice.
 */
export function usePrepareVoice(quiz: Quiz | null, view: GameView | null) {
  const model = useVoiceModel();
  const on = quiz !== null && settingsOf(quiz).readAloud !== "off";
  const lines = useMemo(() => (on && quiz ? questionLines(quiz) : []), [on, quiz]);
  const installed = model?.installed ?? null;
  // Rendering takes CPU the show needs while a question is asked: it waits until the question is over.
  const step = view?.phase.phase === "question" ? view.phase.step : null;
  const asking = useLatest(step === "ready" || step === "running");
  useEffect(() => {
    if (!on) return preparing.set({ state: "off" });
    if (installed === null) return;
    if (!installed) return preparing.set({ state: "noModel" });
    let cancelled = false;
    const total = lines.length;
    void (async () => {
      let failed = 0;
      for (let done = 0; done < total; done++) {
        while (asking.current && !cancelled) await new Promise((resume) => setTimeout(resume, WAIT_WHILE_ASKING_MS));
        if (cancelled) return;
        preparing.set({ state: "rendering", done, total });
        // One clip that fails is read by the system voice; the rest are still rendered.
        await api.voiceClip(lines[done], true).catch(() => failed++);
      }
      if (!cancelled) preparing.set(failed > 0 ? { state: "failed", failed, total } : { state: "ready", total });
    })();
    return () => {
      cancelled = true;
    };
  }, [on, installed, lines, asking]);
}

/** Something that speaks until it ends or is stopped. */
interface Speaking {
  stop(): void;
}

/** Says `line` with its clip at `gain` if one is kept, else with `system`; `onEnd` hears when it ends. */
async function say(line: VoiceLine, gain: number, system: SpeechSynthesisVoice | null | undefined, onEnd: () => void): Promise<Speaking | null> {
  const path = await api.voiceClip(line, false).catch(() => null);
  if (path) {
    const clip = new ClipPlayer(convertFileSrc(path), gain);
    // Played to its end, or a file that does not play: either way it is over.
    clip.onEnd = () => {
      clip.dispose();
      onEnd();
    };
    clip.play({ fromMs: 0, toMs: null });
    return {
      stop: () => {
        clip.onEnd = null;
        clip.dispose();
      },
    };
  }
  const synth = speech();
  if (!synth || !system) return null;
  const utterance = new SpeechSynthesisUtterance(line.text);
  utterance.voice = system;
  utterance.lang = system.lang;
  utterance.volume = Math.min(1, Math.max(0, gain));
  utterance.onend = utterance.onerror = onEnd;
  synth.cancel();
  synth.speak(utterance);
  return {
    stop: () => {
      utterance.onend = utterance.onerror = null;
      synth.cancel();
    },
  };
}

/**
 * The window's one voice: a new line stops the one before. `speaking`
 * holds what reads now (the show's question or the settings' try), so the
 * buttons show it and the music makes way.
 */
const speaker = {
  speaking: createStore<"show" | "preview" | null>(null),
  current: null as Speaking | null,
  turn: 0,

  /**
   * Says `line` as `who`, once `before` (rendering its clip) is done. When
   * `before` fails the line is still said, by the system voice, and the
   * failure is passed on.
   */
  async say(who: "show" | "preview", line: VoiceLine, gain: number, system: SpeechSynthesisVoice | null | undefined, before?: Promise<unknown>) {
    this.stop();
    const mine = ++this.turn;
    this.speaking.set(who);
    const ended = () => {
      if (this.turn !== mine) return;
      this.current = null;
      this.speaking.set(null);
    };
    let failure: unknown = null;
    try {
      await before;
    } catch (error) {
      failure = error ?? new Error("rendering failed");
    }
    if (this.turn !== mine) return;
    const started = await say(line, gain, system, ended);
    if (this.turn !== mine) started?.stop();
    else if (started) this.current = started;
    else ended();
    if (failure !== null) throw failure;
  },

  stop() {
    this.turn++;
    this.current?.stop();
    this.current = null;
    this.speaking.set(null);
  },

  /** Stops what `who` reads, and nothing else. */
  stopIf(who: "show" | "preview") {
    if (this.speaking.get() === who) this.stop();
  },
};

/** Reading out in the Director, for its buttons. */
export interface ReadAloudControl {
  /** The quiz reads questions out and a question is on screen. */
  available: boolean;
  speaking: boolean;
  read: () => void;
  stop: () => void;
}

/**
 * Whether `slide` has sound of its own that the voice would talk over: a
 * sound, however it starts (on the slide, with the clock, which runs as the
 * question shows, or when the quizmaster plays it), or a video that plays
 * by itself and is heard. Such a slide is read only when asked.
 */
export function soundsItself(slide: Slide): boolean {
  const video = slide.video;
  return (slide.sound !== null && slide.sound.volume > 0) || (video !== null && video.autoplay && !video.muted && video.volume > 0);
}

/**
 * Reads the question on screen out when it comes on, as the quiz's
 * settings say, as loud as the show's effects; a buzz, the question's end
 * or its answer stops it, and so does leaving the slide.
 */
export function useReadAloud(view: GameView | null, quiz: Quiz | null): ReadAloudControl {
  const settings = quiz ? settingsOf(quiz) : null;
  const what = settings?.readAloud ?? "off";
  const slide = view?.slide ?? null;
  const line = settings ? voiceLine(slide, settings) : null;
  const system = useSystemVoice(settings?.language ?? "cs");
  const gain = useVoiceGain(quiz);
  const step = view?.phase.phase === "question" ? view.phase.step : null;
  const asking = step === "ready" || step === "running";
  const available = what !== "off" && line !== null && asking;
  const speaking = useStore(speaker.speaking) === "show";

  const read = () => {
    if (available && line) void speaker.say("show", line, gain, system);
  };
  const stop = useCallback(() => speaker.stopIf("show"), []);

  // A question coming on is read once, unless its slide sounds itself.
  const slideIndex = view?.slideIndex ?? null;
  const onQuestion = slide?.type === "question";
  const auto = what !== "off" && slide !== null && !soundsItself(slide);
  const arrived = useLatest(() => auto && read());
  useEffect(() => {
    if (!onQuestion) return;
    arrived.current();
    return stop;
  }, [slideIndex, onQuestion, arrived, stop]);

  // A buzz, the question's end or its answer ends the reading.
  useEffect(() => {
    if (!asking) stop();
  }, [asking, stop]);

  useEffect(() => stop, [stop]);

  return { available, speaking, read, stop };
}

/** How loud the voice reads: as the quiz's effects, at the player's volume. */
export function useVoiceGain(quiz: Quiz | null): number {
  const sounds = quiz ? soundsOf(quiz) : DEFAULT_QUIZ_SOUNDS;
  return linearGain(sounds.effectsVolume, 0) * playerLevel(usePlayerVolume());
}

/** Plays one voice sample at a time, as loud as the player's volume. */
export function useVoiceSample(): {
  playing: string | null;
  play: (language: QuizLanguage, voice: number) => void;
  stop: () => void;
} {
  const clip = useRef<Playing | null>(null);
  const [playing, setPlaying] = useState<string | null>(null);
  const level = useLatest(playerLevel(usePlayerVolume()));
  const stop = useCallback(() => {
    clip.current?.stop();
    clip.current = null;
    setPlaying(null);
  }, []);
  const play = useCallback(
    (language: QuizLanguage, voice: number) => {
      stop();
      const url = sampleUrl(language, voice);
      if (!url) return;
      clip.current = playBundled(url, level.current, stop);
      setPlaying(`${language}-${voice}`);
    },
    [stop, level],
  );
  useEffect(() => stop, [stop]);
  return { playing, play, stop };
}

/** Plays a line once as a show would, rendering its clip first, to try the voice in the settings. */
export function useVoicePreview(): {
  playing: boolean;
  play: (line: VoiceLine, gain: number) => Promise<void>;
  stop: () => void;
} {
  useEffect(watchVoices, []);
  const voices = useLatest(useStore(systemVoices));
  const playing = useStore(speaker.speaking) === "preview";
  const play = useCallback((line: VoiceLine, gain: number) => speaker.say("preview", line, gain, pickSystemVoice(voices.current, line.language), api.voiceClip(line, true)), [voices]);
  const stop = useCallback(() => speaker.stopIf("preview"), []);
  useEffect(() => stop, [stop]);
  return { playing, play, stop };
}
