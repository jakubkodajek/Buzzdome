// The replay part of a question's sound or video (SN-8, QZ-4): played again
// once the answer is revealed, such as the chorus of the song to name.

import { useCallback, useEffect, useRef, type RefObject } from "react";
import { useLatest } from "../hooks";
import type { Replay } from "../types";
import type { PartPlayer } from "./clip-player";

/**
 * Plays `replay`'s part on `player` once `revealed`: what plays stops, and
 * the part starts `replay.delayMs` later, after `onStart`. `source` is the
 * file the player plays, so a player opened later (the media folder known
 * only then) still replays. Returns what cancels a replay still waiting,
 * for when the quizmaster plays or stops the media first.
 */
export function useReplay(player: RefObject<PartPlayer | null>, source: string | null, revealed: boolean, replay: Replay | null, onStart?: () => void): () => void {
  const pending = useRef<ReturnType<typeof setTimeout> | null>(null);
  const started = useLatest(onStart);
  // Views arrive many times a second, so the effect depends on plain values only.
  const [fromMs, toMs, delayMs] = [replay?.part.fromMs ?? 0, replay?.part.toMs ?? null, replay?.delayMs ?? null];

  const cancel = useCallback(() => {
    if (pending.current !== null) clearTimeout(pending.current);
    pending.current = null;
  }, []);

  useEffect(() => {
    const media = player.current;
    if (!media || !revealed || delayMs === null) return;
    media.stop();
    pending.current = setTimeout(() => {
      pending.current = null;
      started.current?.();
      // The player then, should the media have been opened again meanwhile.
      player.current?.play({ fromMs, toMs }, { fromMs });
    }, delayMs);
    return cancel;
  }, [player, source, revealed, fromMs, toMs, delayMs, started, cancel]);

  return cancel;
}
