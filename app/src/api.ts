import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type {
  Button,
  BuzzerId,
  Command,
  Cue,
  DeviceInfo,
  GameView,
  MonitorInfo,
  Quiz,
  ReceiverInfo,
  SessionSummary,
} from "./types";

export const api = {
  getView: () => invoke<GameView | null>("get_view"),
  getQuiz: () => invoke<Quiz | null>("get_quiz"),
  startShowFromFile: (path: string) => invoke<void>("start_show_from_file", { path }),
  startDemoShow: () => invoke<void>("start_demo_show"),
  lastSession: () => invoke<SessionSummary | null>("last_session"),
  resumeLastSession: () => invoke<void>("resume_last_session"),
  endShow: () => invoke<void>("end_show"),
  dispatch: (command: Command) => invoke<void>("dispatch", { command }),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  openHostWindow: (monitor: number | null, fullscreen: boolean) =>
    invoke<void>("open_host_window", { monitor, fullscreen }),
  closeHostWindow: () => invoke<void>("close_host_window"),
  hostWindowOpen: () => invoke<boolean>("host_window_open"),
  receiverInfo: () => invoke<ReceiverInfo>("receiver_info"),
  receiverDevices: () => invoke<DeviceInfo[]>("receiver_devices"),
  receiverSelect: (device: DeviceInfo | null) => invoke<void>("receiver_select", { device }),
  receiverLearnStart: (buzzer: BuzzerId, button: Button) => invoke<void>("receiver_learn_start", { buzzer, button }),
  receiverLearnCancel: () => invoke<void>("receiver_learn_cancel"),
  receiverLearnFinish: () => invoke<string>("receiver_learn_finish"),
  receiverForgetLearned: () => invoke<void>("receiver_forget_learned"),
  receiverExportLog: (path: string) => invoke<void>("receiver_export_log", { path }),
  receiverUdevRule: () => invoke<string>("receiver_udev_rule"),
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

/** The loaded quiz; refetched when a different show starts. */
export function useQuiz(view: GameView | null): Quiz | null {
  const [quiz, setQuiz] = useState<Quiz | null>(null);
  const key = view ? `${view.quizTitle}:${view.slideCount}` : null;
  useEffect(() => {
    if (!key) {
      setQuiz(null);
      return;
    }
    let alive = true;
    api.getQuiz().then((q) => alive && setQuiz(q));
    return () => {
      alive = false;
    };
  }, [key]);
  return quiz;
}

/** Number of this question among all question slides, e.g. 3 of 12. */
export function questionNumber(quiz: Quiz | null, slideIndex: number): { n: number; total: number } | null {
  if (!quiz) return null;
  const questions = quiz.slides.map((s, i) => [s.type, i] as const).filter(([type]) => type === "question");
  const n = questions.findIndex(([, i]) => i === slideIndex);
  return n < 0 ? null : { n: n + 1, total: questions.length };
}
