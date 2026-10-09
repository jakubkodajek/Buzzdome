import { invoke } from "@tauri-apps/api/core";
import type {
  Button,
  BuzzerId,
  ClockView,
  Command,
  Cue,
  DeviceInfo,
  EditorDraft,
  GameView,
  LearnProgress,
  MonitorInfo,
  PressEvent,
  Quiz,
  RawReport,
  ReceiverInfo,
  ReceiverStatus,
  SessionSummary,
  ValidationError,
} from "./types";

/** The backend commands (`#[tauri::command]` in app/src-tauri). */
export const api = {
  getView: () => invoke<GameView | null>("get_view"),
  getQuiz: () => invoke<Quiz | null>("get_quiz"),
  /** Folder of the running quiz's media, ending with a path separator. */
  mediaRoot: () => invoke<string | null>("media_root"),
  startShowFromFile: (path: string) => invoke<void>("start_show_from_file", { path }),
  startDemoShow: () => invoke<void>("start_demo_show"),
  lastSession: () => invoke<SessionSummary | null>("last_session"),
  /** Resumes the last show; returns a warning when something is missing. */
  resumeLastSession: () => invoke<string | null>("resume_last_session"),
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
  /** The quiz editor's draft left from last time, if any. */
  editorLoad: () => invoke<EditorDraft | null>("editor_load"),
  editorNew: (title: string) => invoke<EditorDraft>("editor_new", { title }),
  editorOpen: (path: string) => invoke<EditorDraft>("editor_open", { path }),
  /** Stores the edited quiz; returns what keeps it from being played. */
  editorUpdate: (quiz: Quiz, unsaved: boolean) => invoke<ValidationError[]>("editor_update", { quiz, unsaved }),
  editorSave: (quiz: Quiz, path: string) => invoke<void>("editor_save", { quiz, path }),
  /** Copies a media file into the draft; returns its reference (`media/...`). */
  editorAddMedia: (path: string) => invoke<string>("editor_add_media", { path }),
  editorClose: () => invoke<void>("editor_close"),
  /** Plays the editor's quiz as a show that is not kept for resuming. */
  editorTestRun: (quiz: Quiz) => invoke<void>("editor_test_run", { quiz }),
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
