// Choosing which part of a sound or video plays, and which part it replays
// once the answer is revealed (SN-8): the state, controls and keys the trim
// windows share.

import { useState, type CSSProperties, type KeyboardEvent } from "react";
import { useApp } from "../app-context";
import { WHOLE_FILE } from "../audio/clip-player";
import { MAX_REPLAY_DELAY_MS } from "../bindings/Constants";
import { trackTime } from "../format";
import { isTypingTarget } from "../keyboard";
import type { Replay, Span } from "../types";
import { NumberBox, TimeBox } from "./controls";
import { newReplay } from "./slides";

/** The colours of the part that plays and of the replay part. */
export const PART_COLOR = "#2f6fe4";
export const REPLAY_COLOR = "#1d9a4b";

/** The part colours for the stylesheet, so the Playback and Replay buttons match the parts drawn. */
export const PART_COLORS = { "--trim-part": PART_COLOR, "--trim-replay": REPLAY_COLOR } as CSSProperties;

export interface PartEditing {
  part: Span;
  replay: Replay | null;
  /** The replay part is being edited, not the part that plays. */
  editingReplay: boolean;
  /** The part being edited. */
  editing: Span;
  /** Where a part ends: its own end, or else the end of the file. */
  endOf: (span: Span) => number;
  /** Replaces the part being edited. */
  select: (span: Span) => void;
  /** Starts the part being edited at `ms`; an end at or before it goes. */
  setStart: (ms: number) => void;
  /** Ends the part being edited at `ms`, if that is after its start. */
  setEnd: (ms: number) => void;
  setEditingReplay: (on: boolean) => void;
  setReplay: (replay: Replay | null) => void;
  /**
   * The parts to keep: one that runs to the end of the file says so, so it
   * plays to the end whatever the file's length on another computer.
   */
  result: () => { part: Span; replay: Replay | null };
}

/** The parts of a file `durationMs` long (null until known); a replay part only if `canReplay`. */
export function usePartEditing(initial: { part: Span; replay: Replay | null }, canReplay: boolean, durationMs: number | null): PartEditing {
  const [part, setPart] = useState<Span>(initial.part);
  const [replay, setReplay] = useState<Replay | null>(canReplay ? initial.replay : null);
  const [editingReplay, setEditingReplay] = useState(false);
  const editing = editingReplay && replay ? replay.part : part;
  const select = (span: Span) => (editingReplay && replay ? setReplay({ ...replay, part: span }) : setPart(span));
  const toEnd = (span: Span): Span => (durationMs !== null && span.toMs !== null && span.toMs >= durationMs - 1 ? { ...span, toMs: null } : span);
  return {
    part,
    replay,
    editingReplay,
    editing,
    endOf: (span) => span.toMs ?? durationMs ?? span.fromMs,
    select,
    setStart: (ms) => select({ fromMs: ms, toMs: editing.toMs !== null && editing.toMs <= ms ? null : editing.toMs }),
    setEnd: (ms) => {
      if (ms > editing.fromMs) select({ fromMs: editing.fromMs, toMs: ms });
    },
    setEditingReplay,
    setReplay,
    result: () => ({ part: toEnd(part), replay: replay && { ...replay, part: toEnd(replay.part) } }),
  };
}

/** Whether to replay a part after the reveal, which part is being edited, and the delay. */
export function ReplayChoice({ parts }: { parts: PartEditing }) {
  const { t } = useApp();
  const { replay, editingReplay } = parts;
  return (
    <div className="trim-replay">
      <label>
        <input
          type="checkbox"
          checked={replay !== null}
          onChange={(e) => {
            parts.setReplay(e.target.checked ? newReplay(parts.part) : null);
            parts.setEditingReplay(e.target.checked);
          }}
        />
        {t("propReplay")}
      </label>
      {replay && (
        <>
          <span className="segmented" role="group">
            <button type="button" className="part" aria-pressed={!editingReplay} onClick={() => parts.setEditingReplay(false)}>
              {t("trimPlayback")}
            </button>
            <button type="button" className="replay" aria-pressed={editingReplay} onClick={() => parts.setEditingReplay(true)}>
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
              onChange={(s) => parts.setReplay({ ...replay, delayMs: Math.round(Math.min(MAX_REPLAY_DELAY_MS, Math.max(0, s * 1000))) })}
            />
          </label>
        </>
      )}
    </div>
  );
}

/** The start, end and length of the part being edited, and a button for the whole file. */
export function PartTimes({ parts, durationMs }: { parts: PartEditing; durationMs: number | null }) {
  const { t } = useApp();
  const { editing, endOf, select } = parts;
  const ready = durationMs !== null;
  return (
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
  );
}

/** The trim windows' keys: Space plays or stops, I and O set the start and the end; also on a slider, which takes no typing. */
export function onPartKeys(event: KeyboardEvent, keys: { togglePlay: () => void; setStart: () => void; setEnd: () => void }) {
  if (isTypingTarget(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;
  const run = { " ": keys.togglePlay, i: keys.setStart, o: keys.setEnd }[event.key.toLowerCase()];
  if (!run) return;
  event.preventDefault();
  run();
}
