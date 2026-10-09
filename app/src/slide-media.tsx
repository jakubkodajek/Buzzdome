import { useState } from "react";
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

/** A picture or looping video filling the stage behind the slide. */
export function SlideBackground({ slide, mode = "show" }: { slide: Slide; mode?: ScreenMode }) {
  const toUrl = useMediaUrl();
  const url = toUrl(slide.background);
  if (!url || !slide.background) return null;
  return (
    <div className="slide-background">
      {isVideo(slide.background) ? (
        <video key={url} src={url} autoPlay={mode !== "thumbnail"} muted loop playsInline preload="metadata" />
      ) : (
        <Picture key={url} src={url} />
      )}
    </div>
  );
}

function Picture({ src }: { src: string }) {
  // A missing file shows nothing rather than a broken-image icon on the projector.
  const [failed, setFailed] = useState(false);
  return failed ? null : <img src={src} alt="" onError={() => setFailed(true)} />;
}
