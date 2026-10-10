// Small parts the media player's list and sound board share.

import { useEffect, useState, type DragEvent } from "react";
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

/** The time now, renewed often while `running`, for what plays to count on screen. */
export function useTicking(running: boolean): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 100);
    return () => clearInterval(timer);
  }, [running]);
  return now;
}
