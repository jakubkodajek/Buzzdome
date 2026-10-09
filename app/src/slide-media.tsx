import { useState } from "react";
import { useMediaUrl } from "./media";
import type { Slide } from "./types";

/**
 * The slide's video, or else its picture; nothing if it has neither or the
 * file fails. A `preview` (in the Director) never plays sound on its own.
 */
export function SlideMedia({ slide, className, preview = false }: { slide: Slide; className: string; preview?: boolean }) {
  const toUrl = useMediaUrl();
  const video = slide.video;
  const videoUrl = toUrl(video?.file ?? null);
  const pictureUrl = toUrl(slide.picture);
  if (video && videoUrl) {
    return (
      <div className={className}>
        {preview ? (
          <video key={videoUrl} src={videoUrl} muted controls playsInline />
        ) : (
          <video key={videoUrl} src={videoUrl} autoPlay={video.autoplay} muted={video.muted} loop={video.repeat} playsInline />
        )}
      </div>
    );
  }
  if (pictureUrl) {
    return (
      <div className={className}>
        <Picture key={pictureUrl} src={pictureUrl} />
      </div>
    );
  }
  return null;
}

/** A picture or looping video filling the stage behind the slide. */
export function SlideBackground({ slide }: { slide: Slide }) {
  const toUrl = useMediaUrl();
  const url = toUrl(slide.background);
  if (!url || !slide.background) return null;
  const isVideo = /\.(mp4|webm)$/i.test(slide.background);
  return (
    <div className="slide-background">
      {isVideo ? <video key={url} src={url} autoPlay muted loop playsInline /> : <Picture key={url} src={url} />}
    </div>
  );
}

function Picture({ src }: { src: string }) {
  // A missing file shows nothing rather than a broken-image icon on the projector.
  const [failed, setFailed] = useState(false);
  return failed ? null : <img src={src} alt="" onError={() => setFailed(true)} />;
}
