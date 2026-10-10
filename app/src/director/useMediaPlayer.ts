import { useCallback, useEffect, useRef } from "react";
import { api } from "../api";
import { useRun } from "../app-context";
import { ClipPlayer, playerLevel, WHOLE_FILE } from "../audio/clip-player";
import { useLatest, useMediaLibrary, useNowPlaying, usePlayerVolume, useSoundBoard } from "../hooks";
import { mediaUrl } from "../media";
import { msToEnd, positionAt, showClockNow } from "../media-player";
import type { HeardMedia } from "../media-player";
import type { MediaLibrary, NowPlaying, SoundBoard } from "../types";
import { useMediaDurations } from "./media-durations";

/**
 * How much longer than its length a video may play before it is ended: it
 * plays on the projector, which may start it late (from a slow drive), and
 * cannot tell its own end.
 */
const VIDEO_END_GRACE_MS = 1_000;

export interface MediaPlayer {
  library: MediaLibrary | null;
  board: SoundBoard | null;
  now: NowPlaying | null;
  /** What plays as this window heard it, to count on the show clock from (`showClockNow`). */
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

  // One player per start of a sound. Its own end ends it, however late it started.
  useEffect(() => {
    if (!soundUrl || id === null) return;
    const clip = new ClipPlayer(soundUrl, latestLevel.current);
    clip.onEnd = () => run(api.mediaControl({ type: "ended", id }));
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, [soundUrl, id, latestLevel, run]);

  useEffect(() => player.current?.setGain(level), [level]);

  useEffect(() => {
    const clip = player.current;
    const current = latest.current;
    if (!clip || !current?.now) return;
    if (current.now.sinceMs === null) clip.pause();
    else clip.play(WHOLE_FILE, { fromMs: positionAt(current.now, showClockNow(current)) });
  }, [soundUrl, id, positionMs, sinceMs, latest]);

  // When its length says it ends: a sound that is still playing tells its
  // own end (above); one that does not play, or whose file failed, ends now.
  const durationMs = now ? durationOf(now.reference) : undefined;
  const grace = now?.kind === "video" ? VIDEO_END_GRACE_MS : 0;
  useEffect(() => {
    const current = latest.current;
    const ms = current && msToEnd(current.now, durationMs, showClockNow(current));
    if (ms === null || id === null) return;
    const timer = setTimeout(() => {
      const clip = player.current;
      if (!clip?.playing || clip.failed) run(api.mediaControl({ type: "ended", id }));
    }, ms + grace);
    return () => clearTimeout(timer);
  }, [id, positionMs, sinceMs, durationMs, grace, latest, run]);

  return { library, board, now, heard, durationOf, urlOf };
}
