import { useEffect } from "react";
import { useDispatch } from "./app-context";
import type { Translate } from "./i18n";
import type { Command, GameView } from "./types";

/** The quizmaster's keys, as `KeyboardEvent.key` in lower case. Digits 1-0 are the keyboard buzzers. */
export const KEYS = {
  next: " ",
  right: "y",
  wrong: "n",
  clock: "p",
  leaderboard: "s",
  projector: "m",
} as const;

/** Keys that also go to the next step. */
const NEXT_KEYS: readonly string[] = [KEYS.next, "arrowright", "pagedown"];

/** How a key is written on the button it stands for. */
export function keyHint(t: Translate, key: string): string {
  return key === " " ? t("keySpace") : key.toUpperCase();
}

/** True when a key press is meant for a text field, not for the show. */
export function isTypingTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable);
}

/** True for the keys a running show takes, whether or not the current step has a use for them. */
function isShowKey(key: string): boolean {
  const lower = key.toLowerCase();
  return /^[0-9]$/.test(key) || NEXT_KEYS.includes(lower) || [KEYS.right, KEYS.wrong, KEYS.clock, KEYS.leaderboard].some((k) => k === lower);
}

/**
 * Quizmaster keys, the same in the Director and on the projector window.
 * `onToggleProjector` adds M, which only the Director handles.
 */
export function useShortcuts(view: GameView | null, onToggleProjector?: () => void) {
  const dispatch = useDispatch();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTypingTarget(event.target) || event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key.toLowerCase() === KEYS.projector && onToggleProjector) {
        event.preventDefault();
        onToggleProjector();
        return;
      }
      if (!view || !isShowKey(event.key)) return;
      // Taken even when the step has no use for it, so Space never presses a focused button mid-show.
      event.preventDefault();
      const command = commandForKey(event.key, view);
      if (command) dispatch(command);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, dispatch, onToggleProjector]);
}

/** The command a key stands for, or null when the current step does not take it (see `Controls`). */
export function commandForKey(key: string, view: GameView): Command | null {
  if (/^[0-9]$/.test(key)) {
    // Keyboard buzzers as in the original: 1..9 and 0 for buzzer 10.
    return { type: "press", buzzer: key === "0" ? 10 : Number(key), button: "big" };
  }
  const { controls } = view;
  const lower = key.toLowerCase();
  if (NEXT_KEYS.includes(lower)) return controls.next ? { type: "next" } : null;
  switch (lower) {
    case KEYS.right:
      return controls.judge ? { type: "judgeRight" } : null;
    case KEYS.wrong:
      return controls.judge ? { type: "judgeWrong" } : null;
    case KEYS.clock:
      if (controls.pauseClock) return { type: "pauseClock" };
      return controls.resumeClock ? { type: "resumeClock" } : null;
    case KEYS.leaderboard:
      return { type: "showLeaderboard", visible: !view.leaderboardVisible };
    default:
      return null;
  }
}
