import { useEffect } from "react";
import { api } from "./api";
import type { Command, GameView } from "./types";

/** Quizmaster keys, the same in the Director and on the projector window. */
export function useShortcuts(view: GameView | null, onError: (message: string) => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const command = commandForKey(event.key, view);
      if (!command) return;
      event.preventDefault();
      api.dispatch(command).catch((e) => onError(String(e)));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [view, onError]);
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
