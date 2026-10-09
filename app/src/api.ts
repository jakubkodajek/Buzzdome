import { invoke } from "@tauri-apps/api/core";
import type {
  Button,
  BuzzerId,
  ClockView,
  Command,
  Cue,
  DeviceInfo,
  GameView,
  LearnProgress,
  MonitorInfo,
  PressEvent,
  Quiz,
  RawReport,
  ReceiverInfo,
  ReceiverStatus,
  SessionSummary,
} from "./types";

/** The backend commands (`#[tauri::command]` in app/src-tauri). */
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

/** Events the backend emits (`app.emit` in app/src-tauri), with their payloads. */
export interface BackendEvents {
  /** The game after every accepted command; null when no show runs. */
  "game-state": GameView | null;
  /** A different show (or none) is loaded: reload the quiz. */
  "show-changed": null;
  /** The running question clock, a few times per second. */
  "clock-tick": ClockView;
  "sound-cue": Cue;
  "receiver-status": ReceiverStatus;
  "receiver-raw": RawReport;
  "receiver-press": PressEvent;
  "receiver-learn": LearnProgress;
}
