import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { useApp } from "../app-context";
import { ClipPlayer, inSpan, linearGain, sameSpan, WHOLE_FILE } from "../audio/clip-player";
import { decodeAudioFile } from "../audio/decode";
import { measureLoudness, normalization, type Samples } from "../audio/loudness";
import { trackTime } from "../format";
import { isTypingTarget } from "../keyboard";
import type { Replay, SoundClip, Span } from "../types";
import { Dialog, NumberBox, TimeBox } from "./controls";
import { readLoudnessTarget } from "./normalize";
import { MAX_REPLAY_DELAY_MS, newReplay } from "./slides";
import { panned, waveformPeaks, zoomed, type TimeWindow } from "./waveform";

/** What the dialog changes on the sound. */
export type Trim = Pick<SoundClip, "part" | "replay" | "gainDb">;

interface TrimDialogProps {
  /** The sound file's URL. */
  url: string;
  name: string;
  sound: SoundClip;
  /** The slide is a question, so its sound can replay a part after the reveal. */
  canReplay: boolean;
  onApply: (trim: Trim) => void;
  onClose: () => void;
}

/** Heights in CSS pixels: the waveform, and the bar under it that shows the part not being edited. */
const WAVE_HEIGHT = 150;
const BAR_HEIGHT = 12;
/** How near an edge of the selection a press grabs it. */
const EDGE_PX = 6;
/** A press that moves less than this is a click. */
const CLICK_PX = 3;
const COLORS = {
  background: "#fbfbfb",
  wave: "#4b5563",
  part: "#2f6fe4",
  replay: "#1d9a4b",
  cursor: "#f08a00",
  playhead: "#111111",
};

/** The part colours for the stylesheet, so the Playback and Replay buttons match the waveform. */
const PART_COLORS = { "--trim-part": COLORS.part, "--trim-replay": COLORS.replay } as CSSProperties;

type Drag = { edge: "from" } | { edge: "to" } | { edge: null; anchorMs: number; startX: number; moved: boolean };

/**
 * Sets which part of a sound plays, on its waveform, without changing the
 * file (SN-8): drag to select, click to place the cursor, play the
 * selection (Space) or from the cursor, set the start or end while
 * listening (I, O), zoom with the wheel and pan with Shift. On a question
 * the sound can also replay a part once the answer is revealed. A sound
 * with a loudness gain is measured again over its new part (SN-9).
 */
export function TrimDialog({ url, name, sound, canReplay, onApply, onClose }: TrimDialogProps) {
  const { t } = useApp();
  const [samples, setSamples] = useState<Samples | null>(null);
  const [failed, setFailed] = useState(false);
  const [part, setPart] = useState<Span>(sound.part);
  const [replay, setReplay] = useState<Replay | null>(canReplay ? sound.replay : null);
  const [editingReplay, setEditingReplay] = useState(false);
  const [cursorMs, setCursorMs] = useState(sound.part.fromMs);
  /** The stretch in view; null shows the whole sound. */
  const [view, setView] = useState<TimeWindow | null>(null);
  const [playing, setPlaying] = useState(false);
  const [positionMs, setPositionMs] = useState(0);
  const [width, setWidth] = useState(0);
  const player = useRef<ClipPlayer | null>(null);
  const playingSpan = useRef<Span>(WHOLE_FILE);
  const canvas = useRef<HTMLCanvasElement>(null);
  const drag = useRef<Drag | null>(null);

  const durationMs = samples ? ((samples.channels[0]?.length ?? 0) * 1000) / samples.sampleRate : null;
  const shown = useMemo(() => view ?? { fromMs: 0, toMs: durationMs ?? 0 }, [view, durationMs]);
  const editing = editingReplay && replay ? replay.part : part;
  const endOf = (span: Span) => span.toMs ?? durationMs ?? span.fromMs;
  const select = (span: Span) => (editingReplay && replay ? setReplay({ ...replay, part: span }) : setPart(span));

  useEffect(() => {
    let alive = true;
    decodeAudioFile(url).then(
      (decoded) => alive && setSamples(decoded),
      (error: unknown) => {
        console.error("trim:", error);
        if (alive) setFailed(true);
      },
    );
    return () => {
      alive = false;
    };
  }, [url]);

  // Heard as loud as in the show.
  const gain = linearGain(sound.volume, sound.gainDb);
  useEffect(() => {
    const clip = new ClipPlayer(url, gain);
    clip.onChange = () => setPlaying(clip.playing);
    player.current = clip;
    return () => {
      clip.dispose();
      player.current = null;
    };
  }, [url, gain]);

  useEffect(() => {
    if (!playing) return;
    let frame = requestAnimationFrame(function follow() {
      setPositionMs(player.current?.positionMs ?? 0);
      frame = requestAnimationFrame(follow);
    });
    return () => cancelAnimationFrame(frame);
  }, [playing]);

  useEffect(() => {
    const element = canvas.current?.parentElement;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(Math.floor(entry.contentRect.width)));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  // The wheel zooms around the pointer; Shift or a sideways wheel pans. Not passive, so the dialog does not scroll.
  useEffect(() => {
    const element = canvas.current;
    if (!element || durationMs === null) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = element.getBoundingClientRect();
      setView((current) => {
        const window = current ?? { fromMs: 0, toMs: durationMs };
        const span = window.toMs - window.fromMs;
        const sideways = event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY);
        if (sideways) return panned(window, ((event.shiftKey ? event.deltaY : event.deltaX) / rect.width) * span, durationMs);
        const at = window.fromMs + ((event.clientX - rect.left) / rect.width) * span;
        return zoomed(window, at, event.deltaY > 0 ? 1.25 : 0.8, durationMs);
      });
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, [durationMs]);

  const peaks = useMemo(() => (samples && width > 0 && shown.toMs > shown.fromMs ? waveformPeaks(samples, shown, width) : null), [samples, shown, width]);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context || width === 0) return;
    const ratio = window.devicePixelRatio || 1;
    const height = WAVE_HEIGHT + BAR_HEIGHT;
    element.width = Math.round(width * ratio);
    element.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.fillStyle = COLORS.background;
    context.fillRect(0, 0, width, height);
    const span = shown.toMs - shown.fromMs;
    if (!peaks || span <= 0) return;
    const x = (ms: number) => ((ms - shown.fromMs) / span) * width;
    const end = (s: Span) => s.toMs ?? durationMs ?? s.fromMs;
    const line = (ms: number, color: string) => {
      context.fillStyle = color;
      context.fillRect(Math.round(x(ms)) - 1, 0, 2, WAVE_HEIGHT);
    };
    // The part being edited over the waveform, the other one as a bar under it.
    const parts = [{ span: part, color: COLORS.part, edited: !editingReplay || !replay }];
    if (replay) parts.push({ span: replay.part, color: COLORS.replay, edited: editingReplay });
    for (const { span: p, color, edited } of parts) {
      const [left, right] = [x(p.fromMs), x(end(p))];
      context.fillStyle = edited ? `${color}40` : color;
      if (edited) context.fillRect(left, 0, right - left, WAVE_HEIGHT);
      else context.fillRect(left, WAVE_HEIGHT + 3, right - left, BAR_HEIGHT - 6);
    }
    context.fillStyle = COLORS.wave;
    const middle = WAVE_HEIGHT / 2;
    for (let i = 0; i < peaks.max.length; i++) {
      const top = middle - peaks.max[i] * middle;
      context.fillRect(i, top, 1, Math.max(1, (peaks.max[i] - peaks.min[i]) * middle));
    }
    const edited = parts.find((p) => p.edited) ?? parts[0];
    line(edited.span.fromMs, edited.color);
    line(end(edited.span), edited.color);
    line(cursorMs, COLORS.cursor);
    if (playing) line(positionMs, COLORS.playhead);
  }, [peaks, shown, width, part, replay, editingReplay, cursorMs, playing, positionMs, durationMs]);

  const msAt = (clientX: number) => {
    const rect = canvas.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    const ms = shown.fromMs + ((clientX - rect.left) / rect.width) * (shown.toMs - shown.fromMs);
    return Math.round(Math.min(durationMs ?? 0, Math.max(0, ms)));
  };

  const playSpan = (span: Span, fromMs = span.fromMs) => {
    playingSpan.current = span;
    player.current?.play(span, { fromMs });
  };
  const playSelection = () => playSpan(editing);
  const playFromCursor = () => playSpan({ fromMs: cursorMs, toMs: null });
  const togglePlay = () => (playing ? player.current?.stop() : playSelection());
  /** Where I and O act: the playing position while listening, else the cursor. */
  const markAt = () => Math.round(playing ? (player.current?.positionMs ?? cursorMs) : cursorMs);
  const setStart = () => {
    const at = markAt();
    select({ fromMs: at, toMs: editing.toMs !== null && editing.toMs <= at ? null : editing.toMs });
  };
  const setEnd = () => {
    const at = markAt();
    if (at > editing.fromMs) select({ fromMs: editing.fromMs, toMs: at });
  };

  const onPointerDown = (event: PointerEvent<HTMLCanvasElement>) => {
    if (durationMs === null || width === 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const ms = msAt(event.clientX);
    const near = (EDGE_PX / width) * (shown.toMs - shown.fromMs);
    if (Math.abs(ms - editing.fromMs) <= near) drag.current = { edge: "from" };
    else if (Math.abs(ms - endOf(editing)) <= near) drag.current = { edge: "to" };
    else drag.current = { edge: null, anchorMs: ms, startX: event.clientX, moved: false };
  };
  const onPointerMove = (event: PointerEvent<HTMLCanvasElement>) => {
    const current = drag.current;
    if (!current) return;
    const ms = msAt(event.clientX);
    if (current.edge === "from") select({ fromMs: Math.min(ms, endOf(editing) - 1), toMs: editing.toMs });
    else if (current.edge === "to") select({ fromMs: editing.fromMs, toMs: Math.max(ms, editing.fromMs + 1) });
    else if (current.moved || Math.abs(event.clientX - current.startX) >= CLICK_PX) {
      current.moved = true;
      select({ fromMs: Math.min(current.anchorMs, ms), toMs: Math.max(current.anchorMs + 1, ms) });
    }
  };
  const onPointerUp = () => {
    const current = drag.current;
    drag.current = null;
    if (current?.edge !== null || current.moved) return;
    // A click places the cursor; while playing, playback jumps there.
    setCursorMs(current.anchorMs);
    if (playing) {
      const span = playingSpan.current;
      playSpan(inSpan(span, current.anchorMs) ? span : { fromMs: current.anchorMs, toMs: null }, current.anchorMs);
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (isTypingTarget(event.target) || event.ctrlKey || event.metaKey || event.altKey || durationMs === null) return;
    const run = { " ": togglePlay, i: setStart, o: setEnd }[event.key.toLowerCase()];
    if (!run) return;
    event.preventDefault();
    run();
  };

  const apply = () => {
    // A part that runs to the end of the file plays to its end whatever the file's length on another computer.
    const toEnd = (span: Span): Span => (durationMs !== null && span.toMs !== null && span.toMs >= durationMs - 1 ? { ...span, toMs: null } : span);
    const trimmed = toEnd(part);
    let gainDb = sound.gainDb;
    if (gainDb !== 0 && samples && !sameSpan(trimmed, sound.part)) gainDb = normalization(measureLoudness(samples, trimmed), readLoudnessTarget()).gainDb;
    onApply({ part: trimmed, replay: replay && { ...replay, part: toEnd(replay.part) }, gainDb });
    onClose();
  };

  const ready = durationMs !== null;
  const zoomedIn = ready && view !== null && view.toMs - view.fromMs < durationMs;
  return (
    <Dialog
      title={t("trimTitle", { name })}
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
      <div className="trim" style={PART_COLORS} onKeyDown={onKeyDown}>
        {canReplay && (
          <div className="trim-replay">
            <label>
              <input
                type="checkbox"
                checked={replay !== null}
                onChange={(e) => {
                  setReplay(e.target.checked ? newReplay(part) : null);
                  setEditingReplay(e.target.checked);
                }}
              />
              {t("propReplay")}
            </label>
            {replay && (
              <>
                <span className="segmented" role="group">
                  <button type="button" className="part" aria-pressed={!editingReplay} onClick={() => setEditingReplay(false)}>
                    {t("trimPlayback")}
                  </button>
                  <button type="button" className="replay" aria-pressed={editingReplay} onClick={() => setEditingReplay(true)}>
                    {t("trimReplayPart")}
                  </button>
                </span>
                <label>
                  {t("propReplayDelay")}
                  <NumberBox
                    label={t("propReplayDelay")}
                    value={replay.delayMs / 1000}
                    min={0}
                    max={MAX_REPLAY_DELAY_MS / 1000}
                    step={0.1}
                    unit="s"
                    onChange={(s) => setReplay({ ...replay, delayMs: Math.round(Math.min(MAX_REPLAY_DELAY_MS, Math.max(0, s * 1000))) })}
                  />
                </label>
              </>
            )}
          </div>
        )}
        <div className="trim-wave">
          <canvas
            ref={canvas}
            tabIndex={0}
            data-autofocus
            aria-label={t("trimTitle", { name })}
            style={{ height: WAVE_HEIGHT + BAR_HEIGHT }}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={() => (drag.current = null)}
          />
          {!ready && <p className="trim-status">{failed ? t("trimFailed") : t("loading")}</p>}
        </div>
        {zoomedIn && (
          <input
            type="range"
            className="trim-scroll"
            aria-label={t("trimScroll")}
            min={0}
            max={Math.round(durationMs - (view.toMs - view.fromMs))}
            value={Math.round(view.fromMs)}
            onChange={(e) => setView(panned(view, Number(e.target.value) - view.fromMs, durationMs))}
          />
        )}
        <div className="trim-times">
          <label>
            {t("trimStart")}
            <TimeBox label={t("trimStart")} value={editing.fromMs} max={endOf(editing) - 1} disabled={!ready} onChange={(ms) => select({ fromMs: ms, toMs: editing.toMs })} />
          </label>
          <label>
            {t("trimEnd")}
            <TimeBox
              label={t("trimEnd")}
              value={endOf(editing)}
              min={editing.fromMs + 1}
              max={durationMs ?? 0}
              disabled={!ready}
              onChange={(ms) => select({ fromMs: editing.fromMs, toMs: ms })}
            />
          </label>
          <span>
            {t("trimLength")} <span className="tabular">{trackTime(endOf(editing) - editing.fromMs)}</span>
          </span>
          <button type="button" disabled={!ready} onClick={() => select(WHOLE_FILE)}>
            {t("wholeTrack")}
          </button>
        </div>
        <div className="trim-buttons">
          <button type="button" disabled={!ready} onClick={togglePlay}>
            {playing ? t("stopSound") : t("trimPlaySelection")} <small>{t("keySpace")}</small>
          </button>
          <button type="button" disabled={!ready} onClick={playFromCursor}>
            {t("trimPlayFromCursor")}
          </button>
          <button type="button" disabled={!ready} onClick={setStart}>
            {t("trimSetStart")} <small>I</small>
          </button>
          <button type="button" disabled={!ready} onClick={setEnd}>
            {t("trimSetEnd")} <small>O</small>
          </button>
          <span className="spacer" />
          <button type="button" disabled={!zoomedIn} onClick={() => setView(null)}>
            {t("trimFit")}
          </button>
        </div>
        <p className="trim-hint">{t("trimHint")}</p>
      </div>
    </Dialog>
  );
}
