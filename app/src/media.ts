import { convertFileSrc } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext } from "react";
import { MEDIA_EXTENSIONS } from "./bindings/MediaExtensions";
import type { StringKey, Translate } from "./i18n";
import type { Picker, ThemeFile } from "./types";

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

/** A file that did not tell its length in the time `readMediaLength` gave it. */
export class MediaLengthTimeout extends Error {
  constructor(url: string) {
    super(`${url}: its length was not read in time`);
    this.name = "MediaLengthTimeout";
  }
}

/**
 * The length of a sound or video file in ms, read from its metadata; NaN
 * when it cannot be read or the file tells none (some only know it once
 * played through). Fails with `MediaLengthTimeout` when it does not tell
 * within `timeoutMs`, as a file on a slow drive may not: it may still have one.
 */
export function readMediaLength(url: string, kind: Exclude<MediaKind, "image">, timeoutMs: number): Promise<number> {
  return new Promise((resolve, reject) => {
    const media = document.createElement(kind);
    media.preload = "metadata";
    const timer = setTimeout(() => done(() => reject(new MediaLengthTimeout(url))), timeoutMs);
    function done(settle: () => void) {
      clearTimeout(timer);
      media.onloadedmetadata = media.onerror = null;
      media.removeAttribute("src");
      media.load();
      settle();
    }
    media.onloadedmetadata = () => done(() => resolve(Number.isFinite(media.duration) ? media.duration * 1000 : Number.NaN));
    media.onerror = () => done(() => resolve(Number.NaN));
    media.src = url;
  });
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

/** The wording of a picker for media files of `kinds` (the backend knows their extensions). */
export function mediaPicker(t: Translate, kinds: MediaKind[]): Picker {
  return { filter: kinds.map((kind) => t(MEDIA_FILTER_NAMES[kind])).join(", ") };
}

/** The file name of a media reference, as the quizmaster knows it. */
export function mediaFileName(reference: string): string {
  return reference.replace(/^media\//, "");
}

/** What a media file is, by its extension; null for a type the app does not play. */
export function mediaKindOf(reference: string): MediaKind | null {
  const extension = reference.slice(reference.lastIndexOf(".") + 1).toLowerCase();
  const kinds = Object.keys(MEDIA_EXTENSIONS) as MediaKind[];
  return kinds.find((kind) => (MEDIA_EXTENSIONS[kind] as readonly string[]).includes(extension)) ?? null;
}

/** Whether a media file is a video, by its extension. */
export function isVideo(reference: string): boolean {
  return mediaKindOf(reference) === "video";
}
