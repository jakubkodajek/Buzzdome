import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Command, Cue, GameView, MonitorInfo, SessionSummary } from "./types";

export const api = {
  getView: () => invoke<GameView | null>("get_view"),
  startShowFromFile: (path: string) => invoke<void>("start_show_from_file", { path }),
  startDemoShow: () => invoke<void>("start_demo_show"),
  lastSession: () => invoke<SessionSummary | null>("last_session"),
  resumeLastSession: () => invoke<void>("resume_last_session"),
  dispatch: (command: Command) => invoke<void>("dispatch", { command }),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  openHostWindow: (monitor: number | null, fullscreen: boolean) =>
    invoke<void>("open_host_window", { monitor, fullscreen }),
  closeHostWindow: () => invoke<void>("close_host_window"),
};

/** The latest game state, pushed by the backend after every change. */
export function useGameView(): { view: GameView | null; receivedAt: number } {
  const [state, setState] = useState<{ view: GameView | null; receivedAt: number }>({
    view: null,
    receivedAt: performance.now(),
  });
  useEffect(() => {
    let alive = true;
    const unlisten = listen<GameView | null>("game-state", (event) => {
      if (alive) setState({ view: event.payload, receivedAt: performance.now() });
    });
    api.getView().then((view) => {
      if (alive) setState({ view, receivedAt: performance.now() });
    });
    return () => {
      alive = false;
      unlisten.then((stop) => stop());
    };
  }, []);
  return state;
}

export function useSoundCues(onCue: (cue: Cue) => void) {
  useEffect(() => {
    const unlisten = listen<Cue>("sound-cue", (event) => onCue(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [onCue]);
}

/** Remaining clock time, counted down locally between state updates. */
export function useRemainingMs(view: GameView | null, receivedAt: number): number | null {
  const [now, setNow] = useState(performance.now());
  const clock = view?.clock;
  useEffect(() => {
    if (!clock?.running) return;
    const timer = setInterval(() => setNow(performance.now()), 100);
    return () => clearInterval(timer);
  }, [clock?.running]);
  if (!clock || clock.remainingMs === null) return null;
  if (!clock.running) return clock.remainingMs;
  return Math.max(0, clock.remainingMs - Math.max(0, now - receivedAt));
}
