// A slide's video on the projector (spec QZ-4): the part of the file the
// quiz names, at its speed and volume, starting by itself after its intro
// pause or when the quizmaster plays it, pausing and going on with the show,
// with its effect fading over the answer time and its replay part after the
// reveal.

import { useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import { timeLeft } from "./answer-time";
import { PartPlayer, playerLevel } from "./audio/clip-player";
import { useReplay } from "./audio/replay";
import { useLatest, usePlayerVolume } from "./hooks";
import { heardAtNow, msSinceHeard } from "./media-player";
import type { ClockView, GameView, VideoClip, VideoEffect } from "./types";
import { blurRadius, mosaicColumns } from "./video-effect";

/** One slide's video in the show; give it a new key for each slide. */
export function ShowVideo({ clip, url, view, clock }: { clip: VideoClip; url: string; view: GameView; clock: ClockView | null }) {
  const video = useRef<HTMLVideoElement>(null);
  const mosaic = useRef<HTMLCanvasElement>(null);
  useSlideVideo(video, url, clip, view);
  useVideoEffect(video, mosaic, clip.effect, view, clock);
  return (
    <>
      <video ref={video} src={url} preload="auto" playsInline />
      {clip.effect === "mosaic" && <canvas ref={mosaic} className="video-mosaic" hidden />}
    </>
  );
}

/**
 * Plays the video as the quiz and the show say. It starts `introPauseMs`
 * after the slide appears if it starts by itself, and from the start each
 * time the quizmaster plays it. Once started it pauses while the show holds
 * (`onHold`) and goes on after, unless the quizmaster played it while the
 * show held. Once the answer is revealed it plays its replay part. It
 * sounds at its own volume times the player's volume.
 */
function useSlideVideo(element: RefObject<HTMLVideoElement | null>, url: string, clip: VideoClip, view: GameView) {
  const player = useRef<PartPlayer | null>(null);
  /** The video was started and plays unless the show holds. */
  const started = useRef(false);
  const replaying = useRef(false);
  const latest = useLatest({ clip, onHold: view.onHold });
  const { speed, volume, muted } = clip;
  const level = playerLevel(usePlayerVolume());

  useEffect(() => {
    if (element.current) setPlayback(element.current, { speed, volume: volume * level, muted });
  }, [element, speed, volume, level, muted]);

  // Created first, so the effects below find it on the first render too.
  useEffect(() => {
    const video = element.current;
    if (!video) return;
    const part = new PartPlayer(video);
    player.current = part;
    const { autoplay, introPauseMs } = latest.current.clip;
    const timer = autoplay
      ? setTimeout(() => {
          // The quizmaster may have played it already.
          if (started.current) return;
          started.current = true;
          const { clip: current, onHold } = latest.current;
          if (!onHold) part.play(current.part, { loop: current.repeat, fromMs: current.part.fromMs });
        }, introPauseMs)
      : null;
    return () => {
      if (timer !== null) clearTimeout(timer);
      part.dispose();
      player.current = null;
    };
  }, [element, latest]);

  // The quizmaster's Play video, also one given before this window opened.
  const plays = view.videoPlays;
  useEffect(() => {
    const part = player.current;
    if (!part || plays === 0) return;
    const { clip: current } = latest.current;
    started.current = true;
    replaying.current = false;
    part.play(current.part, { loop: current.repeat, fromMs: current.part.fromMs });
  }, [plays, latest]);

  const onHold = view.onHold;
  useEffect(() => {
    const part = player.current;
    if (!part || !started.current || replaying.current) return;
    const { clip: current } = latest.current;
    if (onHold) part.pause();
    else if (!part.finished) part.play(current.part, { loop: current.repeat });
  }, [onHold, latest]);

  // After the hold above, which a reveal can end in the same view.
  const revealed = view.phase.phase === "question" && view.phase.step === "revealed";
  useReplay(player, url, revealed, clip.replay, () => {
    replaying.current = true;
  });
}

/**
 * Draws the video's effect at the strength the answer time left gives it
 * (`timeLeft`), smoothly between the clock's readings: a blur on the video,
 * or a mosaic drawn on `mosaic` over it.
 */
function useVideoEffect(
  element: RefObject<HTMLVideoElement | null>,
  mosaic: RefObject<HTMLCanvasElement | null>,
  effect: VideoEffect | null,
  view: GameView,
  clock: ClockView | null,
) {
  const reading = useRef({ phase: view.phase, clock, at: 0 });
  useLayoutEffect(() => {
    reading.current = { phase: view.phase, clock, at: heardAtNow() };
  }, [view.phase, clock]);

  useEffect(() => {
    const video = element.current;
    if (!video || !effect) return;
    let frame = requestAnimationFrame(function draw() {
      const { phase, clock: read, at } = reading.current;
      const strength = timeLeft(phase, read, msSinceHeard(at));
      if (effect === "blur") video.style.filter = strength > 0 ? `blur(${blurRadius(strength, video.clientWidth)}px)` : "";
      else if (mosaic.current) drawMosaic(video, mosaic.current, strength);
      frame = requestAnimationFrame(draw);
    });
    return () => {
      cancelAnimationFrame(frame);
      video.style.filter = "";
      video.style.opacity = "";
    };
  }, [element, mosaic, effect]);
}

/**
 * The video's current frame in blocks, over the video's box; the video
 * itself once the blocks would be pixels or before it has a frame.
 */
function drawMosaic(video: HTMLVideoElement, canvas: HTMLCanvasElement, strength: number) {
  const columns = video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA ? mosaicColumns(strength, video.videoWidth) : null;
  canvas.hidden = columns === null;
  video.style.opacity = columns === null ? "" : "0";
  if (columns === null) return;
  const rows = Math.max(1, Math.round((columns * video.videoHeight) / video.videoWidth));
  if (canvas.width !== columns || canvas.height !== rows) {
    canvas.width = columns;
    canvas.height = rows;
  }
  Object.assign(canvas.style, {
    left: `${video.offsetLeft}px`,
    top: `${video.offsetTop}px`,
    width: `${video.offsetWidth}px`,
    height: `${video.offsetHeight}px`,
  });
  canvas.getContext("2d")?.drawImage(video, 0, 0, columns, rows);
}

/** Makes `video` play at a clip's speed and volume, with or without its sound. */
export function setPlayback(video: HTMLVideoElement, { speed, volume, muted }: Pick<VideoClip, "speed" | "volume" | "muted">) {
  video.defaultPlaybackRate = speed / 100;
  video.playbackRate = speed / 100;
  video.volume = volume / 100;
  video.muted = muted;
}
