import { useEffect, useRef, useState } from "react";
import { useApp } from "./app-context";
import { VideoIcon } from "./icons";
import { isVideo, mediaFileName, useMediaUrl } from "./media";
import type { BackgroundLayer } from "./theme/background";
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

/** Files whose background failed in this window; each is reported once. */
const reported = new Set<string>();

/**
 * The background of a stage from its levels (`resolveBackground`): the theme's
 * image, and over it the most specific picture or video that has not failed
 * to load. A failed one gives way to the next level and is reported once
 * (BG-9). In the show a video plays as its background options say: looped or
 * once, silent or not, and held while the show holds (`onHold`) when synced
 * (BG-4). Elsewhere it is silent and stands on its first frame, with a mark
 * that it is a video, unless the editor plays it (`playing`).
 */
export function StageBackground(props: { layers: BackgroundLayer[]; mode?: ScreenMode; onHold?: boolean; playing?: boolean }) {
  const { layers, mode = "show", onHold = false, playing = false } = props;
  const { t, notify } = useApp();
  const toUrl = useMediaUrl();
  const [failed, setFailed] = useState<ReadonlySet<string>>(new Set());
  const theme = layers.find((layer) => layer.level === "theme");
  const media = layers.flatMap((layer) => {
    if (layer.level === "theme") return [];
    const url = toUrl(layer.background.file);
    return url && !failed.has(url) ? [{ background: layer.background, url }] : [];
  })[0];
  const fail = (file: string, url: string) => {
    setFailed((current) => new Set(current).add(url));
    if (reported.has(url)) return;
    reported.add(url);
    notify(t("backgroundFailed", { file: mediaFileName(file) }));
  };
  const show = mode === "show";
  return (
    <>
      {theme && <div className="backdrop" style={{ backgroundImage: `url("${theme.url}")` }} />}
      {media && (
        <div className="slide-background">
          {isVideo(media.background.file) ? (
            <>
              <BackgroundVideo
                key={media.url}
                src={media.url}
                playing={show ? !(media.background.sync && onHold) : playing}
                loop={!show || media.background.repeat}
                muted={!show || media.background.muted}
                preload={show ? "auto" : "metadata"}
                onError={() => fail(media.background.file, media.url)}
              />
              {!show && <VideoIcon className="background-video-mark" />}
            </>
          ) : (
            <img key={media.url} src={media.url} alt="" onError={() => fail(media.background.file, media.url)} />
          )}
        </div>
      )}
    </>
  );
}

function BackgroundVideo(props: { src: string; playing: boolean; loop: boolean; muted: boolean; preload: "auto" | "metadata"; onError: () => void }) {
  const { src, playing, loop, muted, preload, onError } = props;
  const video = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const element = video.current;
    if (!element) return;
    if (playing) element.play().catch((e: unknown) => console.error("background video:", e));
    else element.pause();
  }, [playing]);
  return <video ref={video} src={src} loop={loop} muted={muted} playsInline preload={preload} onError={onError} />;
}

function Picture({ src }: { src: string }) {
  // A missing file shows nothing rather than a broken-image icon on the projector.
  const [failed, setFailed] = useState(false);
  return failed ? null : <img src={src} alt="" onError={() => setFailed(true)} />;
}
