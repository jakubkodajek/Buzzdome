import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type BackendEvents } from "./api";
import type { ClockView, GameView, Quiz, SessionSummary } from "./types";

/** Calls `handler` for every `event` from the backend while mounted. */
export function useBackendEvent<E extends keyof BackendEvents>(event: E, handler: (payload: BackendEvents[E]) => void) {
  // The latest handler runs without re-subscribing on every render.
  const latest = useRef(handler);
  useLayoutEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | null = null;
    listen<BackendEvents[E]>(event, (e) => latest.current(e.payload)).then((unlisten) => {
      if (alive) stop = unlisten;
      else unlisten();
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, [event]);
}

/**
 * Loads a value once on mount and again on every `refreshOn` event. A load
 * that finishes after a newer one started is dropped.
 */
function useBackendValue<T>(load: () => Promise<T>, initial: T, refreshOn: keyof BackendEvents): T {
  const [value, setValue] = useState<T>(initial);
  const generation = useRef(0);
  const reload = useRef(() => {
    const mine = ++generation.current;
    load()
      .then((v) => {
        if (generation.current === mine) setValue(v);
      })
      .catch((e) => console.error(e));
  });
  useEffect(() => {
    reload.current();
    return () => {
      generation.current += 1;
    };
  }, []);
  useBackendEvent(refreshOn, () => reload.current());
  return value;
}

/** The latest game state, pushed by the backend after every change. */
export function useGameView(): GameView | null {
  const [view, setView] = useState<GameView | null>(null);
  const pushed = useRef(false);
  useBackendEvent("game-state", (next) => {
    pushed.current = true;
    setView(next);
  });
  useEffect(() => {
    // The first state comes from a fetch, unless a push already arrived.
    api
      .getView()
      .then((initial) => {
        if (!pushed.current) setView(initial);
      })
      .catch((e) => console.error(e));
  }, []);
  return view;
}

/** The clock of the current question: from the game state, then from ticks. */
export function useClock(view: GameView | null): ClockView | null {
  const viewRef = useRef(view);
  useLayoutEffect(() => {
    viewRef.current = view;
  });
  const [tick, setTick] = useState<{ view: GameView | null; clock: ClockView } | null>(null);
  useBackendEvent("clock-tick", (clock) => setTick({ view: viewRef.current, clock }));
  // A tick belongs to the state it arrived in; a newer state replaces it.
  return tick !== null && tick.view === view ? tick.clock : (view?.clock ?? null);
}

/** The quiz of the running show, reloaded when another show starts. */
export function useQuiz(): Quiz | null {
  return useBackendValue(api.getQuiz, null, "show-changed");
}

/** The show that can be resumed after a crash, if any. */
export function useLastSession(): SessionSummary | null {
  return useBackendValue(api.lastSession, null, "show-changed");
}
