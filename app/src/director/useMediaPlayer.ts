import { useCallback, useEffect, useRef } from "react";
import { api } from "../api";
import { useRun } from "../app-context";
import { ClipPlayer, playerLevel, WHOLE_FILE } from "../audio/clip-player";
import { useLatest, useMediaLibrary, useNowPlaying, usePlayerVolume, useSoundBoard } from "../hooks";
import { mediaUrl } from "../media";
import { msToEnd, positionAt, showClockAt } from "../media-player";
import type { HeardMedia } from "../media-player";
import type { MediaLibrary, NowPlaying, SoundBoard } from "../types";
import { useMediaDurations } from "./media-durations";

export interface MediaPlayer {
  library: MediaLibrary | null;
  board: SoundBoard | null;
  now: NowPlaying | null;
  /** What plays as this window heard it, to count on the show clock from (`showClockAt`). */
  heard: HeardMedia | null;
  /** The length of a library file in ms, once known. */
  durationOf: (reference: string) => number | undefined;
  urlOf: (reference: string) => string | null;
}

/**
 * The media player in the Director (spec SN-7): its library, its board and
 * what plays. Sounds play here, as the window that is open all show; the
 * projector shows pictures and videos. The end of a sound or video is told
 * from here too, so a video ends even with the projector closed.
 */
export function useMediaPlayer(): MediaPlayer {
  const run = useRun();
  const library = useMediaLibrary();
  const board = useSoundBoard();
  const heard = useNowPlaying();
  const now = heard?.now ?? null;
  const level = playerLevel(usePlayerVolume());
  const durations = useMediaDurations(library);
  const root = library?.root ?? null;
  const urlOf = useCallback((reference: string) => (root ? mediaUrl(root, reference) : null), [root]);
  const durationOf = useCallback((reference: string) => durations.get(urlOf(reference) ?? ""), [durations, urlOf]);

  // Effects depend on plain values: a new event with the same values changes nothing.
  const latest = useLatest(heard);
  const id = now?.id ?? null;
  const [positionMs, sinceMs] = [now?.positionMs ?? 0, now?.sinceMs ?? null];
  const soundUrl = now?.kind === "audio" ? urlOf(now.reference) : null;
  const player = useRef<ClipPlayer | null>(null);
  const latestLevel = useLatest(level);

  // One player per start of a sound.
  useEffect(() => {
    if (!soundUrl) return;
    const clip = new ClipPlayer(soundUrl, latestLevel.current);
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, [soundUrl, id, latestLevel]);

  useEffect(() => player.current?.setGain(level), [level]);

  useEffect(() => {
    const clip = player.current;
    const current = latest.current;
    if (!clip || !current?.now) return;
    if (current.now.sinceMs === null) clip.pause();
    else clip.play(WHOLE_FILE, { fromMs: positionAt(current.now, showClockAt(current, performance.now())) });
  }, [soundUrl, id, positionMs, sinceMs, latest]);

  const durationMs = now ? durationOf(now.reference) : undefined;
  useEffect(() => {
    const current = latest.current;
    const ms = current && msToEnd(current.now, durationMs, showClockAt(current, performance.now()));
    if (ms === null || id === null) return;
    const timer = setTimeout(() => run(api.mediaControl({ type: "ended", id })), ms);
    return () => clearTimeout(timer);
  }, [id, positionMs, sinceMs, durationMs, latest, run]);

  return { library, board, now, heard, durationOf, urlOf };
}
