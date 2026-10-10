import { useEffect, useRef } from "react";
import { PartPlayer, playerLevel, WHOLE_FILE } from "../audio/clip-player";
import { positionAt } from "../media-player";
import { useLatest, useMediaLibrary, useNowPlaying, usePlayerVolume } from "../hooks";
import { mediaUrl } from "../media";
import type { NowPlaying } from "../types";

/**
 * A picture or video from the Director's media player (spec SN-7), over the
 * whole stage until it is stopped; sounds play in the Director. A video
 * follows the player: it pauses, goes on and starts over with it, and
 * opening the projector mid-file joins it where it is, and it sounds at the
 * player's volume.
 */
export function MediaOverlay() {
  const library = useMediaLibrary();
  const now = useNowPlaying();
  if (!library || !now || now.kind === "audio") return null;
  const url = mediaUrl(library.root, now.reference);
  return <div className="media-overlay">{now.kind === "image" ? <img src={url} alt="" /> : <OverlayVideo url={url} now={now} />}</div>;
}

function OverlayVideo({ url, now }: { url: string; now: NowPlaying }) {
  const element = useRef<HTMLVideoElement>(null);
  const player = useRef<PartPlayer | null>(null);
  const latest = useLatest(now);
  const { id, positionMs, sinceMs } = now;
  const level = playerLevel(usePlayerVolume());

  useEffect(() => {
    if (element.current) element.current.volume = level;
  }, [url, id, level]);

  useEffect(() => {
    const media = element.current;
    if (!media) return;
    const clip = new PartPlayer(media);
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, [url, id]);

  useEffect(() => {
    const clip = player.current;
    if (!clip) return;
    if (sinceMs === null) clip.pause();
    else clip.play(WHOLE_FILE, { fromMs: positionAt(latest.current, Date.now()) });
  }, [url, id, positionMs, sinceMs, latest]);

  return <video key={`${url}#${id}`} ref={element} src={url} preload="auto" playsInline />;
}
