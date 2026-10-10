import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useApp } from "../app-context";
import { PartPlayer } from "../audio/clip-player";
import { trackTime } from "../format";
import { setPlayback } from "../slide-video";
import type { Replay, Span, VideoClip } from "../types";
import { Dialog } from "./controls";
import { onPartKeys, PART_COLOR, PART_COLORS, PartTimes, REPLAY_COLOR, ReplayChoice, usePartEditing } from "./part-editing";

/** What the dialog changes on the video. */
export type VideoTrim = { part: Span; replay: Replay | null };

interface VideoTrimDialogProps {
  /** The video file's URL. */
  url: string;
  name: string;
  video: VideoClip;
  /** The slide is a question, so its video can replay a part after the reveal. */
  canReplay: boolean;
  onApply: (trim: VideoTrim) => void;
  onClose: () => void;
}

/**
 * Sets which part of a video plays, without changing the file: watch it,
 * move through it with the slider, and set the start or end where it stands
 * (I, O); Space plays the part. On a question the video can also replay a
 * part once the answer is revealed. It plays at the video's speed and volume.
 */
export function VideoTrimDialog({ url, name, video, canReplay, onApply, onClose }: VideoTrimDialogProps) {
  const { t } = useApp();
  const element = useRef<HTMLVideoElement>(null);
  const player = useRef<PartPlayer | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  const [positionMs, setPositionMs] = useState(video.part.fromMs);
  const [playing, setPlaying] = useState(false);
  const parts = usePartEditing(video, canReplay, durationMs);
  const { part, replay, editingReplay, editing, endOf } = parts;
  const { speed, volume, muted } = video;

  useEffect(() => {
    const media = element.current;
    if (!media) return;
    const clip = new PartPlayer(media);
    clip.onChange = () => {
      setPlaying(clip.playing);
      setPositionMs(clip.positionMs);
    };
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, []);

  useEffect(() => {
    if (element.current) setPlayback(element.current, { speed, volume, muted });
  }, [speed, volume, muted]);

  useEffect(() => {
    if (!playing) return;
    let frame = requestAnimationFrame(function follow() {
      setPositionMs(player.current?.positionMs ?? 0);
      frame = requestAnimationFrame(follow);
    });
    return () => cancelAnimationFrame(frame);
  }, [playing]);

  const ready = durationMs !== null;
  const playSpan = (span: Span, fromMs = span.fromMs) => player.current?.play(span, { fromMs });
  const togglePlay = () => (playing ? player.current?.pause() : playSpan(editing));
  const seek = (ms: number) => {
    setPositionMs(ms);
    if (playing) playSpan({ fromMs: ms, toMs: null });
    else if (element.current) element.current.currentTime = ms / 1000;
  };
  const markAt = () => Math.round(player.current?.positionMs ?? positionMs);
  const setStart = () => parts.setStart(markAt());
  const setEnd = () => parts.setEnd(markAt());

  const apply = () => {
    onApply(parts.result());
    onClose();
  };

  // The parts as bands under the slider: the one being edited, and the other one thinner.
  const percent = (ms: number) => (durationMs ? `${(ms / durationMs) * 100}%` : "0%");
  const band = (span: Span, color: string) => `linear-gradient(90deg, transparent ${percent(span.fromMs)}, ${color} ${percent(span.fromMs)}, ${color} ${percent(endOf(span))}, transparent ${percent(endOf(span))})`;
  const bands = { "--edited-band": band(editing, editingReplay ? REPLAY_COLOR : PART_COLOR), "--other-band": replay ? band(editingReplay ? part : replay.part, editingReplay ? PART_COLOR : REPLAY_COLOR) : "none" } as CSSProperties;

  return (
    <Dialog
      title={t("trimVideoTitle", { name })}
      className="trim-dialog"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="primary" disabled={!ready} onClick={apply}>
            {t("ok")}
          </button>
          <button type="button" onClick={onClose}>
            {t("cancel")}
          </button>
        </>
      }
    >
      <div className="trim" style={PART_COLORS} onKeyDown={(e) => ready && onPartKeys(e, { togglePlay, setStart, setEnd })}>
        {canReplay && <ReplayChoice parts={parts} />}
        <div className="trim-video" tabIndex={0} data-autofocus aria-label={t("trimVideoTitle", { name })}>
          <video
            ref={element}
            src={url}
            preload="auto"
            playsInline
            onLoadedMetadata={(e) => {
              setDurationMs(e.currentTarget.duration * 1000);
              e.currentTarget.currentTime = video.part.fromMs / 1000;
            }}
            onError={() => setFailed(true)}
          />
          {!ready && <p className="trim-status">{failed ? t("trimVideoFailed") : t("loading")}</p>}
        </div>
        <div className="trim-seek">
          <div className="trim-track">
            <input
              type="range"
              aria-label={t("trimPosition")}
              min={0}
              max={Math.round(durationMs ?? 0)}
              step={100}
              value={Math.round(positionMs)}
              disabled={!ready}
              onChange={(e) => seek(Number(e.target.value))}
            />
            <div className="trim-bands" style={bands} />
          </div>
          <span className="tabular">{trackTime(positionMs)}</span>
        </div>
        <PartTimes parts={parts} durationMs={durationMs} />
        <div className="trim-buttons">
          <button type="button" disabled={!ready} onClick={togglePlay}>
            {playing ? t("pause") : t("trimPlaySelection")} <small>{t("keySpace")}</small>
          </button>
          <button type="button" disabled={!ready} onClick={() => playSpan({ fromMs: markAt(), toMs: null })}>
            {t("trimPlayFromPosition")}
          </button>
          <button type="button" disabled={!ready} onClick={setStart}>
            {t("trimSetStart")} <small>I</small>
          </button>
          <button type="button" disabled={!ready} onClick={setEnd}>
            {t("trimSetEnd")} <small>O</small>
          </button>
        </div>
        <p className="trim-hint">{t("trimVideoHint")}</p>
      </div>
    </Dialog>
  );
}
