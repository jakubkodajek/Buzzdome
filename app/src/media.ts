import { convertFileSrc } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext } from "react";
import { MEDIA_EXTENSIONS } from "./bindings/MediaExtensions";
import type { StringKey, Translate } from "./i18n";
import type { ThemeFile } from "./types";

export { MEDIA_EXTENSIONS };
export type MediaKind = keyof typeof MEDIA_EXTENSIONS;

/**
 * Folder that media references resolve against, ending with a path
 * separator: the running show's, or the quiz editor's draft.
 */
export const MediaRootContext = createContext<string | null>(null);

/**
 * URL for a file in `<img>`, `<audio>`, `<video>` or a font: a quiz media
 * file (`media/...`) or any other `/`-separated path inside the folder
 * `root`. `root` ends with the platform's path separator, so it is used for
 * the path's `/` too.
 */
export function mediaUrl(root: string, reference: string): string {
  const separator = root.slice(-1);
  return convertFileSrc(root + reference.split("/").join(separator));
}

/** URL for a file of a theme package. */
export function themeFileUrl(file: ThemeFile): string {
  return mediaUrl(file.root, file.path);
}

/** Turns media references into URLs (see `MediaRootContext`); null without a folder. */
export function useMediaUrl(): (reference: string | null) => string | null {
  const root = useContext(MediaRootContext);
  return useCallback((reference) => (root && reference ? mediaUrl(root, reference) : null), [root]);
}

const MEDIA_FILTER_NAMES: Record<MediaKind, StringKey> = {
  image: "mediaPicture",
  audio: "mediaSound",
  video: "mediaVideo",
};

/** A file dialog's filter for media files of `kinds`. */
export function mediaFileFilter(t: Translate, kinds: MediaKind[]): { name: string; extensions: string[] } {
  const extensions = kinds.flatMap((kind) => MEDIA_EXTENSIONS[kind]);
  // Upper case too: some systems match file names case-sensitively.
  return { name: kinds.map((kind) => t(MEDIA_FILTER_NAMES[kind])).join(", "), extensions: [...extensions, ...extensions.map((e) => e.toUpperCase())] };
}

/** The file name of a media reference, as the quizmaster knows it. */
export function mediaFileName(reference: string): string {
  return reference.replace(/^media\//, "");
}

/** Whether a media file is a video, by its extension. */
export function isVideo(reference: string): boolean {
  const extension = reference.slice(reference.lastIndexOf(".") + 1).toLowerCase();
  return (MEDIA_EXTENSIONS.video as readonly string[]).includes(extension);
}
