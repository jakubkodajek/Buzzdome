import { useEffect, useRef } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { PartPlayer, playerLevel, WHOLE_FILE } from "../audio/clip-player";
import { positionAt, showClockNow, type HeardMedia } from "../media-player";
import { useLatest, useMediaLibrary, useNowPlaying, usePlayerVolume } from "../hooks";
import { mediaFileName, mediaUrl } from "../media";
import type { NowPlaying } from "../types";

/**
 * A picture or video from the Director's media player (spec SN-7), over the
 * whole stage until it is stopped; sounds play in the Director. A video
 * follows the player: it pauses, goes on and starts over with it, and
 * opening the projector mid-file joins it where it is, and it sounds at the
 * player's volume. Its end, or its file failing, is told from here: only
 * this window can tell. A file that fails stops, so the stage does not stay
 * black, and the Director is told.
 */
export function MediaOverlay() {
  const { t, notify } = useApp();
  const run = useRun();
  const library = useMediaLibrary();
  const heard = useNowPlaying();
  const now = heard?.now;
  if (!library || !heard || !now || now.kind === "audio") return null;
  const url = mediaUrl(library.root, now.reference);
  const ended = (failed: boolean) => {
    if (failed) notify(t("playerFileFailed", { file: mediaFileName(now.reference) }));
    run(api.mediaEnded(now.id));
  };
  return (
    <div className="media-overlay">
      {now.kind === "image" ? <img key={url} src={url} alt="" onError={() => ended(true)} /> : <OverlayVideo url={url} now={now} heard={heard} onEnd={ended} />}
    </div>
  );
}

function OverlayVideo({ url, now, heard, onEnd }: { url: string; now: NowPlaying; heard: HeardMedia; onEnd: (failed: boolean) => void }) {
  const element = useRef<HTMLVideoElement>(null);
  const player = useRef<PartPlayer | null>(null);
  const latest = useLatest(heard);
  const latestEnd = useLatest(onEnd);
  const { id, positionMs, sinceMs } = now;
  const level = playerLevel(usePlayerVolume());

  useEffect(() => {
    if (element.current) element.current.volume = level;
  }, [url, id, level]);

  useEffect(() => {
    const media = element.current;
    if (!media) return;
    // It stays on its last frame until the player stops it.
    const clip = new PartPlayer(media, { holdAtEnd: true });
    clip.onEnd = () => latestEnd.current(clip.failed);
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, [url, id, latestEnd]);

  useEffect(() => {
    const clip = player.current;
    if (!clip) return;
    const current = latest.current;
    if (sinceMs === null || !current.now) clip.pause();
    else clip.play(WHOLE_FILE, { fromMs: positionAt(current.now, showClockNow(current)) });
  }, [url, id, positionMs, sinceMs, latest]);

  return <video key={`${url}#${id}`} ref={element} src={url} preload="auto" playsInline />;
}
