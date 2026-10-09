import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback } from "react";
import { useMediaRoot } from "./hooks";

/**
 * URL for a quiz media file (`media/...`) in `<img>`, `<audio>` or `<video>`.
 * `root` ends with the platform's path separator, so it is used for the
 * reference's `/` too.
 */
export function mediaUrl(root: string, reference: string): string {
  const separator = root.slice(-1);
  return convertFileSrc(root + reference.split("/").join(separator));
}

/** Turns media references of the running quiz into URLs; null without a quiz folder. */
export function useMediaUrl(): (reference: string | null) => string | null {
  const root = useMediaRoot();
  return useCallback((reference) => (root && reference ? mediaUrl(root, reference) : null), [root]);
}
