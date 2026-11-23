// The files the AI panel sends a model on this computer (AiPanel.tsx):
// which files it takes, and each as the model gets it. A picture goes
// smaller, a video as a few frames: both taken in the window from the
// file's own data, so nothing is read from the disk but what the
// quizmaster gave. A video plays from a `blob:` URL of its data, which the
// content policy allows for media (tauri.conf.json); pictures need none.

import { mediaKindOf } from "../media";
import type { Attachment, LocalModelInfo } from "../types";

export type AiFileKind = "picture" | "video" | "text";

/** Text files the panel reads as they are. */
const TEXT_EXTENSIONS = ["txt", "md", "csv"];

/** A file in the panel. */
export interface AiFile {
  /** Tells files of the same name apart. */
  id: number;
  file: File;
  kind: AiFileKind;
  /** The picture or video copied into the draft for the slides, once it is. */
  copied: { draft: string; reference: string } | null;
}

/** What the panel does with a file of `name`; null for one it cannot read. */
export function aiFileKind(name: string): AiFileKind | null {
  const extension = name.toLowerCase().split(".").pop() ?? "";
  if (TEXT_EXTENSIONS.includes(extension)) return "text";
  switch (mediaKindOf(name)) {
    case "image":
      return "picture";
    case "video":
      return "video";
    default:
      return null;
  }
}

let nextId = 1;

/**
 * `files` added to `current`, at most `max` in all: those it cannot read
 * are named in `unsupported`, and `tooMany` says some were left out. A file
 * already there (same name, size and time) is not added again.
 */
export function addFiles(current: readonly AiFile[], files: readonly File[], max: number): { files: AiFile[]; unsupported: string[]; tooMany: boolean } {
  const next = [...current];
  const unsupported: string[] = [];
  let tooMany = false;
  for (const file of files) {
    const kind = aiFileKind(file.name);
    if (kind === null) {
      unsupported.push(file.name);
      continue;
    }
    if (next.some((had) => sameFile(had.file, file))) continue;
    if (next.length >= max) {
      tooMany = true;
      continue;
    }
    next.push({ id: nextId++, file, kind, copied: null });
  }
  return { files: next, unsupported, tooMany };
}

function sameFile(a: File, b: File): boolean {
  return a.name === b.name && a.size === b.size && a.lastModified === b.lastModified;
}

/** The size of a `width` × `height` picture that fits in `side` × `side`, never larger than it is. */
export function fitWithin(width: number, height: number, side: number): { width: number; height: number } {
  const scale = Math.min(1, side / Math.max(width, height, 1));
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

/** When to take `count` frames of a video `duration` seconds long: the middles of equal parts, so neither black start nor end. */
export function frameTimes(duration: number, count: number): number[] {
  if (!Number.isFinite(duration) || duration <= 0 || count <= 0) return [0];
  return Array.from({ length: count }, (_, i) => ((i + 0.5) * duration) / count);
}

/** Qualities a JPEG is tried at until it fits the size a request takes. */
const QUALITIES = [0.85, 0.7, 0.5];

/** `source` drawn at most `side` pixels wide and high, as a JPEG data URL of at most `maxBytes`; null if none fits. */
function drawn(source: CanvasImageSource, width: number, height: number, side: number, maxBytes: number): string | null {
  const size = fitWithin(width, height, side);
  const canvas = document.createElement("canvas");
  canvas.width = size.width;
  canvas.height = size.height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  // A transparent picture on white, as JPEG has no transparency.
  context.fillStyle = "#fff";
  context.fillRect(0, 0, size.width, size.height);
  context.drawImage(source, 0, 0, size.width, size.height);
  for (const quality of QUALITIES) {
    const url = canvas.toDataURL("image/jpeg", quality);
    if (url.length <= maxBytes) return url;
  }
  return null;
}

/** The picture `file` decoded, without a URL the window's content policy would have to allow. */
async function decoded(file: File): Promise<ImageBitmap> {
  return createImageBitmap(file);
}

async function pictureFor(file: File, info: LocalModelInfo): Promise<string> {
  const bitmap = await decoded(file);
  try {
    const picture = drawn(bitmap, bitmap.width, bitmap.height, info.imageSidePx, info.maxImageBytes);
    if (!picture) throw new Error("the picture cannot be drawn");
    return picture;
  } finally {
    bitmap.close();
  }
}

/** Longest side of a picture's thumbnail in the panel, in CSS pixels at twice their density. */
const THUMBNAIL_PX = 96;

/** A small copy of the picture `file` for the panel's list; null when it cannot be decoded. */
export async function thumbnailOf(file: File): Promise<string | null> {
  try {
    const bitmap = await decoded(file);
    try {
      return drawn(bitmap, bitmap.width, bitmap.height, THUMBNAIL_PX, Number.MAX_SAFE_INTEGER);
    } finally {
      bitmap.close();
    }
  } catch {
    return null;
  }
}

/** Waits for `event` on `target`, failing on an error or after `ms`. */
function once(target: HTMLMediaElement, event: string, ms: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const done = (ok: boolean) => {
      clearTimeout(timer);
      target.removeEventListener(event, onEvent);
      target.removeEventListener("error", onError);
      if (ok) resolve();
      else reject(new Error(`the video gave no ${event}`));
    };
    const onEvent = () => done(true);
    const onError = () => done(false);
    const timer = setTimeout(() => done(false), ms);
    target.addEventListener(event, onEvent);
    target.addEventListener("error", onError);
  });
}

/** How long a video may take to open or to show a frame. */
const VIDEO_WAIT_MS = 15_000;

/**
 * Frames of the video `file` for the model; none when this window cannot
 * play it (a codec the system lacks), and the model then goes by its name.
 */
async function framesFor(file: File, info: LocalModelInfo): Promise<string[]> {
  const url = URL.createObjectURL(file);
  const video = document.createElement("video");
  video.muted = true;
  video.preload = "auto";
  video.playsInline = true;
  try {
    video.src = url;
    await once(video, "loadeddata", VIDEO_WAIT_MS);
    const frames: string[] = [];
    for (const time of frameTimes(video.duration, info.videoFrames)) {
      // A video already at that time fires no `seeked`.
      if (video.currentTime !== time) {
        video.currentTime = time;
        await once(video, "seeked", VIDEO_WAIT_MS);
      }
      const frame = drawn(video, video.videoWidth, video.videoHeight, info.imageSidePx, info.maxImageBytes);
      if (frame) frames.push(frame);
    }
    return frames;
  } catch {
    return [];
  } finally {
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(url);
  }
}

/**
 * `file` as the model gets it, with `reference`, the draft's copy of a
 * picture or video. `textLeft` is how much text the request still takes.
 */
export async function attachmentFor(file: AiFile, info: LocalModelInfo, reference: string | null, textLeft: number): Promise<Attachment> {
  const name = file.file.name;
  switch (file.kind) {
    case "picture":
      return { name, media: reference, content: { kind: "picture", image: await pictureFor(file.file, info) } };
    case "video":
      return { name, media: reference, content: { kind: "video", frames: await framesFor(file.file, info) } };
    case "text": {
      // Read only up to what is sent (at most four bytes a character); a huge log file is not read in full.
      const bytes = new Uint8Array(await file.file.slice(0, Math.max(0, textLeft) * 4).arrayBuffer());
      return { name, media: null, content: { kind: "text", text: decodedText(bytes).slice(0, textLeft) } };
    }
  }
}

/**
 * The text in `bytes`: UTF-8, or UTF-16 with its byte order mark, as text
 * editors save it; otherwise Windows-1250, in which Czech Windows and Excel
 * save a CSV. The last character may be cut, as only the start is read.
 */
export function decodedText(bytes: Uint8Array): string {
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder("utf-16le").decode(bytes);
  if (bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder("utf-16be").decode(bytes);
  try {
    // `stream` lets a character cut at the end pass; the decoder drops it.
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes, { stream: true });
  } catch {
    return new TextDecoder("windows-1250").decode(bytes);
  }
}
