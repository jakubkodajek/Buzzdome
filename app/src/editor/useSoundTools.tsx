import { useState, type ReactNode } from "react";
import { useApp } from "../app-context";
import { sameSpan } from "../audio/clip-player";
import { MAX_DECODED_MINUTES } from "../audio/decode";
import { decibels } from "../format";
import { mediaFileName, mediaUrl } from "../media";
import type { Slide } from "../types";
import type { OpenQuiz } from "./editor-state";
import type { Edit } from "./history";
import { LoudnessDialog } from "./LoudnessDialog";
import { LOUDNESS_STATUS_KEYS, measureSound, normalizedGain, readLoudnessTarget, unreadSound } from "./normalize";
import { setSoundOptions, setVideoOptions } from "./slides";
import { TrimDialog } from "./TrimDialog";
import { VideoTrimDialog } from "./VideoTrimDialog";

export interface SoundTools {
  /** Opens the trim window for a slide's sound or video. */
  trim: (id: string, media: "sound" | "video") => void;
  /** Measures a slide's sound and sets its loudness gain to the remembered target (SN-9). */
  normalize: (id: string) => void;
  /** Opens the window that evens out every sound of the quiz. */
  normalizeAll: () => void;
  /** Whether one of the windows is open, which then takes the keys. */
  windowOpen: boolean;
  /** The open window, to draw in the editor. */
  windows: ReactNode;
}

/**
 * The editor's tools for a slide's sound and video: the trim windows and
 * evening out loudness, one sound or all of them. It holds which window is
 * open and draws it.
 */
export function useSoundTools(open: OpenQuiz | null, edit: (edit: Edit, mergeKey?: string) => void): SoundTools {
  const { t, notify } = useApp();
  const [trimOf, setTrimOf] = useState<{ id: string; media: "sound" | "video" } | null>(null);
  const [loudness, setLoudness] = useState(false);
  const quiz = open?.history.present ?? null;
  const slides = quiz?.slides ?? [];
  const trimSlide = trimOf === null ? undefined : slides.find((s) => s.id === trimOf.id);
  const updateSlide = (id: string, update: (s: Slide) => Slide) => edit({ type: "updateSlide", id, update });

  const normalize = (id: string) => {
    const sound = slides.find((s) => s.id === id)?.sound;
    if (!open || !sound) return;
    measureSound(mediaUrl(open.mediaRoot, sound.file), sound).then(
      (measured) => {
        const change = normalizedGain(sound.gainDb, measured, readLoudnessTarget());
        const { gainDb } = change;
        // Only while the sound still plays the part that was measured.
        if (gainDb !== null) updateSlide(id, (s) => (s.sound?.file === sound.file && sameSpan(s.sound.part, sound.part) ? setSoundOptions(s, { gainDb }) : s));
        notify(t(change.message, { gain: decibels(gainDb ?? sound.gainDb) }));
      },
      (error: unknown) => {
        console.error("normalize:", error);
        notify(t(LOUDNESS_STATUS_KEYS[unreadSound(error)], { minutes: MAX_DECODED_MINUTES }));
      },
    );
  };

  const windows = open && (
    <>
      {trimOf?.media === "sound" && trimSlide?.sound && (
        <TrimDialog
          key={trimSlide.id}
          url={mediaUrl(open.mediaRoot, trimSlide.sound.file)}
          name={mediaFileName(trimSlide.sound.file)}
          sound={trimSlide.sound}
          canReplay={trimSlide.type === "question"}
          onApply={(trim) => updateSlide(trimSlide.id, (s) => setSoundOptions(s, trim))}
          onClose={() => setTrimOf(null)}
        />
      )}
      {trimOf?.media === "video" && trimSlide?.video && (
        <VideoTrimDialog
          key={trimSlide.id}
          url={mediaUrl(open.mediaRoot, trimSlide.video.file)}
          name={mediaFileName(trimSlide.video.file)}
          video={trimSlide.video}
          canReplay={trimSlide.type === "question"}
          onApply={(trim) => updateSlide(trimSlide.id, (s) => setVideoOptions(s, trim))}
          onClose={() => setTrimOf(null)}
        />
      )}
      {loudness && quiz && (
        <LoudnessDialog
          quiz={quiz}
          mediaRoot={open.mediaRoot}
          onApply={(gains) => edit({ type: "batch", edits: [...gains].map(([id, gainDb]) => ({ type: "updateSlide", id, update: (s: Slide) => setSoundOptions(s, { gainDb }) })) })}
          onClose={() => setLoudness(false)}
        />
      )}
    </>
  );

  return {
    trim: (id, media) => setTrimOf({ id, media }),
    normalize,
    normalizeAll: () => setLoudness(true),
    windowOpen: Boolean(trimSlide) || loudness,
    windows,
  };
}
