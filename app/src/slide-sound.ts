import { useCallback, useEffect, useRef, useState } from "react";
import { useMediaUrl } from "./media";
import type { GameView } from "./types";

export interface SlideSound {
  /** The current slide has a sound that can be played. */
  available: boolean;
  playing: boolean;
  play: () => void;
  stop: () => void;
}

/**
 * Plays the current slide's sound clip: when the slide opens, while the
 * question clock runs, or when the quizmaster asks, as the quiz says.
 */
export function useSlideSound(view: GameView | null): SlideSound {
  const toUrl = useMediaUrl();
  const sound = view?.slide?.sound ?? null;
  const url = toUrl(sound?.file ?? null);
  const slideIndex = view?.slideIndex ?? null;
  const start = sound?.start ?? null;
  const repeat = sound?.repeat ?? false;
  const volume = sound?.volume ?? 100;
  const audio = useRef<HTMLAudioElement | null>(null);
  const [playing, setPlaying] = useState(false);

  // One audio element per slide; leaving the slide stops it.
  useEffect(() => {
    if (!url) return;
    const element = new Audio(url);
    element.loop = repeat;
    element.volume = volume / 100;
    const update = () => setPlaying(!element.paused);
    element.addEventListener("play", update);
    element.addEventListener("pause", update);
    element.addEventListener("ended", update);
    audio.current = element;
    if (start === "onSlide") play(element);
    return () => {
      element.pause();
      audio.current = null;
    };
  }, [url, slideIndex, start, repeat, volume]);

  // "With the clock": runs and pauses with the question clock.
  const step = view?.phase.phase === "question" ? view.phase.step : null;
  const clockRunning = step === "running" && (view?.clock?.running ?? false);
  useEffect(() => {
    const element = audio.current;
    if (!element || start !== "withClock") return;
    if (clockRunning) play(element);
    else element.pause();
  }, [clockRunning, start, url, slideIndex]);

  const playNow = useCallback(() => {
    if (audio.current) play(audio.current);
  }, []);
  const stop = useCallback(() => {
    const element = audio.current;
    if (!element) return;
    element.pause();
    element.currentTime = 0;
  }, []);

  return { available: url !== null, playing: url !== null && playing, play: playNow, stop };
}

function play(element: HTMLAudioElement) {
  element.play().catch((e) => console.error("sound:", e));
}
