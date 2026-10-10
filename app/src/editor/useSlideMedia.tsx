// The media of the slides in the editor: choosing a picture, video, sound
// or background file for them, changing or removing it, trimming and
// evening out the sound, and playing a background video on the canvas.

import { useState } from "react";
import { useApp } from "../app-context";
import { BackgroundIcon, FolderIcon, LoudnessIcon, PictureIcon, PlayIcon, SoundIcon, StopIcon, TrashIcon, TrimIcon, VideoIcon } from "../icons";
import { isVideo, type MediaKind } from "../media";
import { plays } from "../quiz";
import type { Slide } from "../types";
import type { Action, SlideMediaActionId } from "./actions";
import { isMediaElement, type ElementId, type MediaElement } from "./elements";
import { setBackground, setPicture, setSound, setVideo } from "./slides";
import type { QuizEditor } from "./useQuizEditor";
import type { SlideList } from "./useSlideList";
import type { SoundTools } from "./useSoundTools";

/** How each media part of a slide is chosen and set. */
const MEDIA: Record<MediaElement, { kinds: MediaKind[]; set: (slide: Slide, file: string | null) => Slide }> = {
  picture: { kinds: ["image"], set: setPicture },
  video: { kinds: ["video"], set: setVideo },
  sound: { kinds: ["audio"], set: setSound },
};

export interface SlideMedia {
  /** Asks for a file of `kinds` and puts it on the slides `ids` with `apply`. */
  choose: (ids: string[], kinds: MediaKind[], apply: (s: Slide, file: string) => Slide) => void;
  /** Asks for the picture, video or sound `part` of the slide `id`, and selects it once it is there. */
  choosePart: (id: string, part: MediaElement) => void;
  /** The background video of the slide on the canvas plays there (it shows its first frame otherwise). */
  playingBackground: boolean;
  actions: Record<SlideMediaActionId, Action>;
}

export function useSlideMedia(editor: QuizEditor, list: SlideList, sound: SoundTools): SlideMedia {
  const { t } = useApp();
  const [playingBackgroundOf, setPlayingBackgroundOf] = useState<string | null>(null);
  const { slides, slide, selected, selectedSlides, element, update, setSelection } = list;
  const media = isMediaElement(element) ? element : null;
  const backgroundVideo = slide?.background ? isVideo(slide.background.file) : false;
  const playingBackground = backgroundVideo && playingBackgroundOf === slide?.id;

  /** Asks for a file and puts it on the slides; `part` is selected once it is there (one slide). */
  const pick = (ids: string[], kinds: MediaKind[], apply: (s: Slide, file: string) => Slide, part?: ElementId) =>
    void editor.pickMedia(kinds).then((file) => {
      if (!file) return;
      update(ids, (s) => apply(s, file));
      if (part && ids.length === 1) setSelection((current) => (current.id === ids[0] ? { ...current, element: part } : current));
    });
  const choosePart = (id: string, part: MediaElement) => pick([id], MEDIA[part].kinds, MEDIA[part].set, part);

  const actions: Record<SlideMediaActionId, Action> = {
    insertPicture: {
      label: t("mediaPicture"),
      icon: <PictureIcon />,
      disabled: !slide || !plays(slide.type).pictureAndVideo,
      run: () => slide && choosePart(slide.id, "picture"),
    },
    insertVideo: {
      label: t("mediaVideo"),
      icon: <VideoIcon />,
      disabled: !slide || !plays(slide.type).pictureAndVideo,
      run: () => slide && choosePart(slide.id, "video"),
    },
    insertSound: { label: t("mediaSound"), icon: <SoundIcon />, disabled: !slide, run: () => slide && choosePart(slide.id, "sound") },
    insertBackground: {
      label: t("mediaBackground"),
      icon: <BackgroundIcon />,
      disabled: !slide,
      // A background goes on every selected slide.
      run: () => pick(selected, ["image", "video"], setBackground),
    },
    changeMedia: { label: t("changeFile"), icon: <FolderIcon />, disabled: !media, run: () => slide && media && choosePart(slide.id, media) },
    removeMedia: {
      label: t("remove"),
      icon: <TrashIcon />,
      keys: media ? ["Delete", "Backspace"] : undefined,
      disabled: !media,
      run: () => slide && media && update([slide.id], (s) => MEDIA[media].set(s, null)),
    },
    trimSound: { label: t("trimAction"), icon: <TrimIcon />, disabled: !slide?.sound, run: () => slide && sound.trim(slide.id, "sound") },
    trimVideo: { label: t("trimAction"), icon: <TrimIcon />, disabled: !slide?.video, run: () => slide && sound.trim(slide.id, "video") },
    normalizeSound: { label: t("normalizeAction"), icon: <LoudnessIcon />, disabled: !slide?.sound, run: () => slide && sound.normalize(slide.id) },
    normalizeAll: { label: t("normalizeAll"), icon: <LoudnessIcon />, disabled: !slides.some((s) => s.sound), run: sound.normalizeAll },
    toggleBackground: {
      label: t(playingBackground ? "stopBackground" : "playBackground"),
      icon: playingBackground ? <StopIcon /> : <PlayIcon />,
      disabled: !backgroundVideo,
      run: () => slide && setPlayingBackgroundOf(playingBackground ? null : slide.id),
    },
    removeBackground: {
      label: t("removeBackground"),
      icon: <TrashIcon />,
      disabled: !selectedSlides.some((s) => s.background),
      run: () => update(selected, (s) => setBackground(s, null)),
    },
  };

  return { choose: (ids, kinds, apply) => pick(ids, kinds, apply), choosePart, playingBackground, actions };
}
