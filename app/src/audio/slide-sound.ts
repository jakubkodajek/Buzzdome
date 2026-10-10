import { useCallback, useEffect, useRef, useState } from "react";
import { useMediaUrl } from "../media";
import type { GameView, Span } from "../types";
import { ClipPlayer, linearGain } from "./clip-player";
import { useReplay } from "./replay";

export interface SlideSound {
  /** The current slide has a sound that can be played. */
  available: boolean;
  playing: boolean;
  play: () => void;
  stop: () => void;
}

/**
 * Plays the current slide's sound clip: its part (SN-8) when the slide
 * opens, while the question clock runs, or when the quizmaster asks, as the
 * quiz says; and its replay part once the answer is revealed.
 */
export function useSlideSound(view: GameView | null): SlideSound {
  const toUrl = useMediaUrl();
  const sound = view?.slide?.sound ?? null;
  const url = toUrl(sound?.file ?? null);
  const slideIndex = view?.slideIndex ?? null;
  // Views arrive many times a second, so effects depend on plain values only.
  const start = sound?.start ?? null;
  const repeat = sound?.repeat ?? false;
  const gain = sound ? linearGain(sound.volume, sound.gainDb) : 1;
  const [fromMs, toMs] = [sound?.part.fromMs ?? 0, sound?.part.toMs ?? null];
  const player = useRef<ClipPlayer | null>(null);
  const [playing, setPlaying] = useState(false);

  // One player per slide; leaving the slide stops it.
  useEffect(() => {
    if (!url) return;
    const clip = new ClipPlayer(url, gain);
    clip.onChange = () => setPlaying(clip.playing);
    player.current = clip;
    if (start === "onSlide") clip.play(part(fromMs, toMs), { loop: repeat });
    return () => {
      clip.dispose();
      player.current = null;
      setPlaying(false);
    };
  }, [url, slideIndex, start, repeat, gain, fromMs, toMs]);

  // "With the clock": runs and pauses with the question clock.
  const step = view?.phase.phase === "question" ? view.phase.step : null;
  const clockRunning = step === "running" && (view?.clock?.running ?? false);
  useEffect(() => {
    const clip = player.current;
    if (!clip || start !== "withClock") return;
    if (clockRunning) clip.play(part(fromMs, toMs), { loop: repeat });
    else clip.pause();
  }, [clockRunning, start, repeat, fromMs, toMs, url, slideIndex]);

  // The replay part, once the answer is on screen.
  const cancelReplay = useReplay(player, url, step === "revealed", sound?.replay ?? null);

  const playNow = useCallback(() => {
    cancelReplay();
    player.current?.play(part(fromMs, toMs), { loop: repeat });
  }, [fromMs, toMs, repeat, cancelReplay]);
  const stop = useCallback(() => {
    cancelReplay();
    player.current?.stop();
  }, [cancelReplay]);

  return { available: url !== null, playing: url !== null && playing, play: playNow, stop };
}

function part(fromMs: number, toMs: number | null): Span {
  return { fromMs, toMs };
}
