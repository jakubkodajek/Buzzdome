// Small parts the media player's list and sound board share.

import { useEffect, useState, type DragEvent } from "react";
import { useLatest } from "../hooks";
import { showClockAt, type HeardMedia } from "../media-player";
import { PictureIcon, SoundIcon, VideoIcon } from "../icons";
import type { MediaKind } from "../types";

/** Drag data of a library file: its reference. */
export const LIBRARY_FILE_TYPE = "application/x-quiz-show-media";

export function KindIcon({ kind }: { kind: MediaKind }) {
  return kind === "image" ? <PictureIcon /> : kind === "audio" ? <SoundIcon /> : <VideoIcon />;
}

/** Whether a drag carries files from the system's file manager. */
export function carriesFiles(e: DragEvent): boolean {
  return e.dataTransfer.types.includes("Files");
}

/**
 * The show clock now, counted on from what plays as this window heard it,
 * renewed often while it plays, for what plays to count on screen.
 */
export function useTicking(media: HeardMedia | null): number {
  const latest = useLatest(media);
  const [now, setNow] = useState(() => (media ? showClockAt(media, performance.now()) : 0));
  const running = (media?.now?.sinceMs ?? null) !== null;
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => {
      if (latest.current) setNow(showClockAt(latest.current, performance.now()));
    }, 100);
    return () => clearInterval(timer);
  }, [running, latest]);
  return now;
}
