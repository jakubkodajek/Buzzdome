import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type BackendEvents } from "./api";
import type { ClockView, GameView, LearnProgress, Quiz, SessionSummary } from "./types";

/** A ref that always holds the latest `value`, for callbacks that outlive a render. */
export function useLatest<T>(value: T): { readonly current: T } {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  });
  return ref;
}

/** Calls `handler` for every `event` from the backend while mounted. */
export function useBackendEvent<E extends keyof BackendEvents>(event: E, handler: (payload: BackendEvents[E]) => void) {
  // The latest handler runs without re-subscribing on every render.
  const latest = useLatest(handler);
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
  }, [event, latest]);
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

/**
 * A value the backend sends with `event` whenever it changes. The first value
 * comes from `load`, unless a push already arrived; null until then.
 */
function usePushedValue<E extends keyof BackendEvents>(event: E, load: () => Promise<BackendEvents[E]>): BackendEvents[E] | null {
  const [value, setValue] = useState<BackendEvents[E] | null>(null);
  const pushed = useRef(false);
  const initial = useRef(load);
  useBackendEvent(event, (next) => {
    pushed.current = true;
    setValue(next);
  });
  useEffect(() => {
    initial
      .current()
      .then((first) => {
        if (!pushed.current) setValue(first);
      })
      .catch((e) => console.error(e));
  }, []);
  return value;
}

/** The latest game state, pushed by the backend after every change. */
export function useGameView(): GameView | null {
  return usePushedValue("game-state", api.getView);
}

/** Whether a buzzer button is being learned; presses do not play meanwhile. */
export function useLearnProgress(): LearnProgress | null {
  return usePushedValue("receiver-learn", () => api.receiverInfo().then((info) => info.learn));
}

/** The clock of the current question: from the game state, then from ticks. */
export function useClock(view: GameView | null): ClockView | null {
  const viewRef = useLatest(view);
  const [tick, setTick] = useState<{ view: GameView | null; clock: ClockView } | null>(null);
  useBackendEvent("clock-tick", (clock) => setTick({ view: viewRef.current, clock }));
  // A tick belongs to the state it arrived in; a newer state replaces it.
  return tick !== null && tick.view === view ? tick.clock : (view?.clock ?? null);
}

/** The quiz of the running show, reloaded when another show starts. */
export function useQuiz(): Quiz | null {
  return useBackendValue(api.getQuiz, null, "show-changed");
}

/** Folder of the running quiz's media (see `media.ts`). */
export function useMediaRoot(): string | null {
  return useBackendValue(api.mediaRoot, null, "show-changed");
}

/** The show that can be resumed after a crash, if any. */
export function useLastSession(): SessionSummary | null {
  return useBackendValue(api.lastSession, null, "show-changed");
}
