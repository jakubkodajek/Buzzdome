// The lengths of the media library's sounds and videos, read from their
// files' metadata one file at a time, so a full library does not load
// dozens of files at once.

import { useEffect } from "react";
import { mediaUrl, readMediaLength } from "../media";
import { createStore, useStore, type Store } from "../store";
import type { MediaKind, MediaLibrary } from "../types";

export type TimedKind = Exclude<MediaKind, "image">;

export interface MediaDurations {
  /** Length in ms by file URL, as they become known; NaN for a file whose length cannot be read. */
  lengths: Store<ReadonlyMap<string, number>>;
  /**
   * Measures the listed files not measured yet, one after another, and
   * forgets the ones no longer listed, so a new file of the same name is
   * measured again.
   */
  follow: (files: ReadonlyMap<string, TimedKind>) => void;
}

/** Lengths measured with `read`, which tells a file's length (NaN for none) and never fails. */
export function mediaDurations(read: (url: string, kind: TimedKind) => Promise<number>): MediaDurations {
  const lengths = createStore<ReadonlyMap<string, number>>(new Map());
  const queued = new Set<string>();
  let listed: ReadonlyMap<string, TimedKind> = new Map();
  let queue = Promise.resolve();

  const measure = (url: string, kind: TimedKind) => {
    queued.add(url);
    queue = queue
      .then(() => read(url, kind))
      .then((ms) => {
        queued.delete(url);
        // A file that left the list while it waited is not kept.
        if (listed.has(url)) lengths.set(new Map(lengths.get()).set(url, ms));
      });
  };

  return {
    lengths,
    follow: (files) => {
      listed = files;
      const known = lengths.get();
      const kept = new Map([...known].filter(([url]) => files.has(url)));
      if (kept.size < known.size) lengths.set(kept);
      for (const [url, kind] of files) if (!kept.has(url) && !queued.has(url)) measure(url, kind);
    },
  };
}

const durations = mediaDurations(readMediaLength);

/** The lengths of the library's sounds and videos by URL, as they become known. Used once, by the Director. */
export function useMediaDurations(library: MediaLibrary | null): ReadonlyMap<string, number> {
  useEffect(() => {
    if (!library) return;
    const timed = library.files.flatMap((file) => (file.kind === "image" ? [] : [[mediaUrl(library.root, file.reference), file.kind] as const]));
    durations.follow(new Map(timed));
  }, [library]);
  return useStore(durations.lengths);
}
