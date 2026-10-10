import { useEffect, useRef, useState } from "react";
import { VideoIcon } from "./icons";
import { isVideo, useMediaUrl } from "./media";
import type { Slide } from "./types";

/**
 * Where a slide is drawn: on the projector during the show, as a preview
 * the quizmaster works with (videos wait for a click and stay muted), or as
 * a small thumbnail (videos show their first frame).
 */
export type ScreenMode = "show" | "preview" | "thumbnail";

/** The slide's video, or else its picture; nothing if it has neither or the file fails. */
export function SlideMedia({ slide, className, mode = "show" }: { slide: Slide; className: string; mode?: ScreenMode }) {
  const toUrl = useMediaUrl();
  const video = slide.video;
  const videoUrl = toUrl(video?.file ?? null);
  const pictureUrl = toUrl(slide.picture);
  if (video && videoUrl) {
    return (
      <div className={className} data-element="video">
        {mode === "show" ? (
          <video key={videoUrl} src={videoUrl} autoPlay={video.autoplay} muted={video.muted} loop={video.repeat} playsInline />
        ) : (
          <video key={videoUrl} src={videoUrl} muted controls={mode === "preview"} preload="metadata" playsInline />
        )}
      </div>
    );
  }
  if (pictureUrl) {
    return (
      <div className={className} data-element="picture">
        <Picture key={pictureUrl} src={pictureUrl} />
      </div>
    );
  }
  return null;
}

/**
 * The slide's own picture or video, filling the stage behind its content.
 * In the show a video plays as its background options say: looped or once,
 * silent or not, and held while the show holds (`onHold`) when synced
 * (BG-4). Elsewhere it is silent and stands on its first frame, with a mark
 * that it is a video, unless the editor plays it (`playing`).
 */
export function SlideBackground(props: { slide: Slide; mode?: ScreenMode; onHold?: boolean; playing?: boolean }) {
  const { slide, mode = "show", onHold = false, playing = false } = props;
  const toUrl = useMediaUrl();
  const background = slide.background;
  const url = toUrl(background?.file ?? null);
  if (!url || !background) return null;
  const show = mode === "show";
  return (
    <div className="slide-background">
      {isVideo(background.file) ? (
        <>
          <BackgroundVideo
            key={url}
            src={url}
            playing={show ? !(background.sync && onHold) : playing}
            loop={!show || background.repeat}
            muted={!show || background.muted}
            preload={show ? "auto" : "metadata"}
          />
          {!show && <VideoIcon className="background-video-mark" />}
        </>
      ) : (
        <Picture key={url} src={url} />
      )}
    </div>
  );
}

function BackgroundVideo(props: { src: string; playing: boolean; loop: boolean; muted: boolean; preload: "auto" | "metadata" }) {
  const { src, playing, loop, muted, preload } = props;
  const video = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const element = video.current;
    if (!element) return;
    if (playing) element.play().catch((e: unknown) => console.error("background video:", e));
    else element.pause();
  }, [playing]);
  return <video ref={video} src={src} loop={loop} muted={muted} playsInline preload={preload} />;
}

function Picture({ src }: { src: string }) {
  // A missing file shows nothing rather than a broken-image icon on the projector.
  const [failed, setFailed] = useState(false);
  return failed ? null : <img src={src} alt="" onError={() => setFailed(true)} />;
}
