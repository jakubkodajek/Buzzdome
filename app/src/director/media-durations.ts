// The lengths of the media library's sounds and videos, read from their
// files' metadata one file at a time, so a full library does not load
// dozens of files at once.

import { useEffect, useSyncExternalStore } from "react";
import { mediaUrl, readMediaLength } from "../media";
import type { MediaLibrary, MediaKind } from "../types";

/** Length in ms by file URL; NaN for a file whose length cannot be read. */
const lengths = new Map<string, number>();
const queued = new Set<string>();
const listeners = new Set<() => void>();
let version = 0;
let queue: Promise<void> = Promise.resolve();

function measure(url: string, kind: Exclude<MediaKind, "image">) {
  if (lengths.has(url) || queued.has(url)) return;
  queued.add(url);
  queue = queue
    .then(() => readMediaLength(url, kind))
    .then((ms) => {
      queued.delete(url);
      lengths.set(url, ms);
      version += 1;
      listeners.forEach((listener) => listener());
    });
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * The lengths of the library's sounds and videos by URL, as they become
 * known. A file that leaves the library is forgotten, so a new file of the
 * same name is measured again. Used once, by the Director.
 */
export function useMediaDurations(library: MediaLibrary | null): ReadonlyMap<string, number> {
  useSyncExternalStore(subscribe, () => version);
  useEffect(() => {
    if (!library) return;
    const urls = new Set<string>();
    for (const file of library.files) {
      const url = mediaUrl(library.root, file.reference);
      urls.add(url);
      if (file.kind !== "image") measure(url, file.kind);
    }
    for (const url of lengths.keys()) if (!urls.has(url)) lengths.delete(url);
  }, [library]);
  return lengths;
}
