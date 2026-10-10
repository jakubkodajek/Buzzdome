import { useContext, useEffect, useRef, useState } from "react";
import { useApp } from "./app-context";
import { VideoIcon } from "./icons";
import { isVideo, mediaFileName, MediaRootContext, themeFileUrl, useMediaUrl } from "./media";
import { ShowVideo } from "./slide-video";
import type { BackgroundLayer } from "./theme/background";
import type { Background, ClockView, GameView, Slide } from "./types";

/**
 * Where a slide is drawn: on the projector during the show, as a preview
 * the quizmaster works with (videos wait for a click and stay muted), or as
 * a small thumbnail (videos show their first frame).
 */
export type ScreenMode = "show" | "preview" | "thumbnail";

/** The game state a slide's video plays along with in the show. */
export interface LiveShow {
  view: GameView;
  clock: ClockView | null;
}

/**
 * The slide's video, or else its picture; nothing if it has neither or the
 * file fails. In the show (`live`) the video plays along with the show
 * (`ShowVideo`).
 */
export function SlideMedia(props: { slide: Slide; className: string; mode?: ScreenMode; live?: LiveShow }) {
  const { slide, className, mode = "show", live } = props;
  const toUrl = useMediaUrl();
  const video = slide.video;
  const videoUrl = toUrl(video?.file ?? null);
  const pictureUrl = toUrl(slide.picture);
  if (video && videoUrl) {
    return (
      <div className={className} data-element="video">
        {mode === "show" && live ? (
          <ShowVideo key={`${live.view.slideIndex}:${videoUrl}`} clip={video} url={videoUrl} view={live.view} clock={live.clock} />
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
 * Files whose background failed in this window, with the media folder open
 * then; each is reported once while that folder is open.
 */
const reported = new Set<string>();

/** The backgrounds that failed with the media folder `root`. */
interface Failed {
  root: string | null;
  urls: ReadonlySet<string>;
}

/** A theme's background video loops silently, as a new slide background does (BG-4). */
const THEME_VIDEO: Pick<Background, "repeat" | "muted" | "sync"> = { repeat: true, muted: true, sync: false };

/** A level's file, ready to draw. */
interface Drawn {
  level: BackgroundLayer["level"];
  /** The file as the quizmaster knows it, for the warning when it fails. */
  name: string;
  url: string;
  video: boolean;
  options: Pick<Background, "repeat" | "muted" | "sync">;
}

/**
 * The background of a stage from its levels (`resolveBackground`): the theme's
 * picture or video, and over it the most specific one that has not failed
 * to load. A failed one gives way to the next level and is reported once
 * (BG-9). In the show a video plays as its background options say: looped or
 * once, silent or not, and held while the show holds (`onHold`) when synced
 * (BG-4). Elsewhere it is silent and stands on its first frame, with a mark
 * on the slide's own that it is a video, unless the editor plays it
 * (`playing`).
 */
export function StageBackground(props: { layers: BackgroundLayer[]; mode?: ScreenMode; onHold?: boolean; playing?: boolean }) {
  const { layers, mode = "show", onHold = false, playing = false } = props;
  const { t, notify } = useApp();
  const toUrl = useMediaUrl();
  // Another show or draft tries every file again, so a file mended since loads.
  const root = useContext(MediaRootContext);
  const [failed, setFailed] = useState<Failed>({ root, urls: new Set() });
  const failedNow = failed.root === root ? failed.urls : null;
  const drawn = layers.flatMap((layer): Drawn[] => {
    if (layer.level === "theme") {
      const { path } = layer.file;
      return [{ level: "theme", name: path, url: themeFileUrl(layer.file), video: isVideo(path), options: THEME_VIDEO }];
    }
    const { background } = layer;
    const url = toUrl(background.file);
    return url ? [{ level: "slide", name: mediaFileName(background.file), url, video: isVideo(background.file), options: background }] : [];
  });
  const loading = drawn.filter((item) => !failedNow?.has(item.url));
  const theme = loading.find((item) => item.level === "theme");
  const own = loading.find((item) => item.level !== "theme");
  const show = mode === "show";
  const draw = (item: Drawn, playsHere: boolean) => {
    const fail = () => {
      setFailed((current) => ({ root, urls: new Set(current.root === root ? current.urls : []).add(item.url) }));
      const once = `${root}\n${item.url}`;
      if (reported.has(once)) return;
      reported.add(once);
      notify(t(item.level === "theme" ? "themeBackgroundFailed" : "backgroundFailed", { file: item.name }));
    };
    if (!item.video) return <img key={item.url} src={item.url} alt="" onError={fail} />;
    return (
      <BackgroundVideo
        key={item.url}
        src={item.url}
        playing={show ? !(item.options.sync && onHold) : playsHere}
        loop={!show || item.options.repeat}
        muted={!show || item.options.muted}
        preload={show ? "auto" : "metadata"}
        onError={fail}
      />
    );
  };
  return (
    <>
      {theme && <div className="backdrop">{draw(theme, false)}</div>}
      {own && (
        <div className="slide-background">
          {draw(own, playing)}
          {!show && own.video && <VideoIcon className="background-video-mark" />}
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
