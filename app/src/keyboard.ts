import { useEffect } from "react";
import { useDispatch } from "./app-context";
import type { Command, GameView } from "./types";

/** True when a key press is meant for a text field, not for the show. */
export function isTypingTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable);
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
      if (event.key.toLowerCase() === "m" && onToggleProjector) {
        event.preventDefault();
        onToggleProjector();
        return;
      }
      const command = commandForKey(event.key, view);
      if (!command) return;
      event.preventDefault();
      dispatch(command);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, dispatch, onToggleProjector]);
}

function commandForKey(key: string, view: GameView | null): Command | null {
  if (/^[0-9]$/.test(key)) {
    // Keyboard buzzers as in the original: 1..9 and 0 for buzzer 10.
    return { type: "press", buzzer: key === "0" ? 10 : Number(key), button: "big" };
  }
  switch (key.toLowerCase()) {
    case " ":
    case "arrowright":
    case "pagedown":
      return { type: "next" };
    case "y":
      return { type: "judgeRight" };
    case "n":
      return { type: "judgeWrong" };
    case "p":
      return view?.clock?.running ? { type: "pauseClock" } : { type: "resumeClock" };
    case "s":
      return { type: "showLeaderboard", visible: !view?.leaderboardVisible };
    default:
      return null;
  }
}
