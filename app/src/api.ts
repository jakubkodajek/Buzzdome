import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  BoardChange,
  Button,
  BuzzerId,
  Category,
  ClockView,
  Command,
  Cue,
  DeviceInfo,
  EditorDraft,
  GameView,
  IconColor,
  LearnProgress,
  ListSeparator,
  MediaLibrary,
  MonitorInfo,
  NowPlaying,
  PlayerCommand,
  Press,
  PressEvent,
  Quiz,
  RawReport,
  ReceiverInfo,
  ReceiverStatus,
  SessionSummary,
  SettingsLost,
  SheetExport,
  SheetImport,
  SoundBoard,
  Theme,
  ValidationError,
} from "./types";

/** The header that names a file dropped into the window (`media_add_dropped`). */
const DROPPED_NAME_HEADER = "file-name";

/** The backend commands (`#[tauri::command]` in app/src-tauri). */
export const api = {
  getView: () => invoke<GameView | null>("get_view"),
  getQuiz: () => invoke<Quiz | null>("get_quiz"),
  /** Folder of the running quiz's media, ending with a path separator. */
  mediaRoot: () => invoke<string | null>("media_root"),
  startShowFromFile: (path: string) => invoke<void>("start_show_from_file", { path }),
  startDemoShow: () => invoke<void>("start_demo_show"),
  lastSession: () => invoke<SessionSummary | null>("last_session"),
  /** Resumes the last show; returns warnings when it runs but not quite as before (its media are missing, other rules). */
  resumeLastSession: () => invoke<AppError[]>("resume_last_session"),
  endShow: () => invoke<void>("end_show"),
  dispatch: (command: Command) => invoke<void>("dispatch", { command }),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  openHostWindow: (monitor: number | null, fullscreen: boolean) =>
    invoke<void>("open_host_window", { monitor, fullscreen }),
  closeHostWindow: () => invoke<void>("close_host_window"),
  hostWindowOpen: () => invoke<boolean>("host_window_open"),
  /** Every installed theme with all its values, the default theme among them. */
  listThemes: () => invoke<Theme[]>("list_themes"),
  /** The family names of the installed fonts, sorted; empty when they cannot be read. */
  installedFonts: () => invoke<string[]>("installed_fonts"),
  /** The team names prepared for shows (the running show's while one runs). */
  preparedTeams: () => invoke<string[]>("prepared_teams"),
  teamsLost: () => invoke<SettingsLost | null>("teams_lost"),
  addTeams: (names: string[]) => invoke<void>("add_teams", { names }),
  removeTeam: (name: string) => invoke<void>("remove_team", { name }),
  /** The media player's library of files, kept on this computer. */
  mediaLibrary: () => invoke<MediaLibrary>("media_library"),
  /** Copies files into the library; returns their references in order. */
  mediaAdd: (paths: string[]) => invoke<string[]>("media_add", { paths }),
  /** Copies a file dropped into the window into the library; returns its reference. */
  mediaAddDropped: async (file: File) =>
    invoke<string>("media_add_dropped", new Uint8Array(await file.arrayBuffer()), { headers: { [DROPPED_NAME_HEADER]: encodeURIComponent(file.name) } }),
  /** Removes a file from the library and deletes its copy. */
  mediaRemove: (reference: string) => invoke<void>("media_remove", { reference }),
  soundBoard: () => invoke<SoundBoard>("sound_board"),
  soundBoardChange: (change: BoardChange) => invoke<void>("sound_board_change", { change }),
  /** What the media player plays, if anything. */
  mediaNow: () => invoke<NowPlaying | null>("media_now"),
  mediaControl: (command: PlayerCommand) => invoke<void>("media_control", { command }),
  /** The player's volume in percent, the level everything the show plays sounds at. */
  mediaVolume: () => invoke<number>("media_volume"),
  mediaSetVolume: (volume: number) => invoke<void>("media_set_volume", { volume }),
  /** The colour of the windows' icon. */
  appIcon: () => invoke<IconColor>("app_icon"),
  setAppIcon: (color: IconColor) => invoke<void>("set_app_icon", { color }),
  receiverInfo: () => invoke<ReceiverInfo>("receiver_info"),
  receiverDevices: () => invoke<DeviceInfo[]>("receiver_devices"),
  receiverSelect: (device: DeviceInfo | null) => invoke<void>("receiver_select", { device }),
  receiverLearnStart: (buzzer: BuzzerId, button: Button) => invoke<void>("receiver_learn_start", { buzzer, button }),
  receiverLearnCancel: () => invoke<void>("receiver_learn_cancel"),
  /** Saves the button being learned and returns it. */
  receiverLearnFinish: () => invoke<Press>("receiver_learn_finish"),
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
  /**
   * Reads questions from a spreadsheet, copying their media into the draft,
   * for the editor to insert; a row's `CAT` names one of `categories`.
   */
  editorImportSheet: (path: string, categories: readonly Category[]) => invoke<SheetImport>("editor_import_sheet", { path, categories }),
  /** Writes the quiz as a CSV file with its media next to it. */
  editorExportSheet: (quiz: Quiz, path: string, separator: ListSeparator) =>
    invoke<SheetExport>("editor_export_sheet", { quiz, path, separator }),
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
  /** The projector window opened (true) or closed (false). */
  "host-window": boolean;
  /** The prepared team names changed, or a show started or ended. */
  "teams-changed": string[];
  /** Files came into the media library or left it. */
  "media-library": MediaLibrary;
  "sound-board": SoundBoard;
  /** The media player started, paused or stopped something. */
  "media-playing": NowPlaying | null;
  /** The player's volume changed, in percent. */
  "media-volume": number;
  /** The windows' icon took another colour. */
  "app-icon": IconColor;
}
