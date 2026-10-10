import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type BackendEvents } from "./api";
import { MAX_PLAYER_VOLUME } from "./bindings/Constants";
import type { ClockView, GameView, IconColor, LearnProgress, MediaLibrary, NowPlaying, Quiz, SessionSummary, SettingsLost, SoundBoard } from "./types";

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

/** Whether the projector window is open, however it was opened or closed. */
export function useHostWindowOpen(): boolean {
  return usePushedValue("host-window", api.hostWindowOpen) ?? false;
}

/** Whether a buzzer button is being learned; presses do not play meanwhile. */
export function useLearnProgress(): LearnProgress | null {
  return usePushedValue("receiver-learn", () => api.receiverInfo().then((info) => info.learn));
}

/** The team names prepared for shows, kept between them; empty until loaded. */
export function usePreparedTeams(): string[] {
  return usePushedValue("teams-changed", api.preparedTeams) ?? [];
}

/** The media player's library; null until loaded. */
export function useMediaLibrary(): MediaLibrary | null {
  return usePushedValue("media-library", api.mediaLibrary);
}

/** The sound board's pads; null until loaded. */
export function useSoundBoard(): SoundBoard | null {
  return usePushedValue("sound-board", api.soundBoard);
}

/** What the media player plays, if anything. */
export function useNowPlaying(): NowPlaying | null {
  return usePushedValue("media-playing", api.mediaNow);
}

/** The colour of the windows' icon; null until loaded. */
export function useAppIcon(): IconColor | null {
  return usePushedValue("app-icon", api.appIcon);
}

/** The player's volume in percent, everything the show plays sounds at; full until loaded. */
export function usePlayerVolume(): number {
  return usePushedValue("media-volume", api.mediaVolume) ?? MAX_PLAYER_VOLUME;
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

/** Why the saved team names could not be read when the app started, if so. */
export function useTeamsLost(): SettingsLost | null {
  return useBackendValue(api.teamsLost, null, "teams-changed");
}

/** The show that can be resumed after a crash, if any, or why the last show cannot be read. */
export interface LastSession {
  session: SessionSummary | null;
  /** The error `last_session` gave (an `AppError`, for `errorText`); null when it read. */
  problem: unknown;
}

const NO_SESSION: LastSession = { session: null, problem: null };

export function useLastSession(): LastSession {
  const load = () => api.lastSession().then(
    (session): LastSession => ({ session, problem: null }),
    (problem: unknown): LastSession => ({ session: null, problem }),
  );
  return useBackendValue(load, NO_SESSION, "show-changed");
}
