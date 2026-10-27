import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  BoardChange,
  Button,
  BuzzerId,
  BuzzerSeen,
  Category,
  ClaudeConnected,
  ClockView,
  Command,
  Cue,
  DeviceInfo,
  EditorDraft,
  GameView,
  IconColor,
  LearnProgress,
  LightsDevices,
  LibraryMatches,
  LibraryInfo,
  LightsInfo,
  LightsSetup,
  LightsStatus,
  ListSeparator,
  MediaKind,
  MediaLibrary,
  MediaNow,
  MenuWords,
  MonitorInfo,
  Picker,
  ProjectorWindow,
  PlayerCommand,
  Press,
  PressEvent,
  Quiz,
  RawReport,
  ReceiverInfo,
  ReceiverStatus,
  RemoteAction,
  RemoteKey,
  RemoteKeyAction,
  SessionSummary,
  SettingsLost,
  SheetExport,
  SheetImport,
  ShowResults,
  SoundBoard,
  Theme,
  ValidationError,
} from "./types";

/** The header that names a file dropped into the window (`media_add_dropped`). */
const DROPPED_NAME_HEADER = "file-name";

/** The backend commands (`#[tauri::command]` in app/src-tauri). */
export const api = {
  getView: () => invoke<GameView | null>("get_view"),
  /** The show's results so far (spec SF-11). */
  showResults: () => invoke<ShowResults | null>("show_results"),
  /** Writes `rows` of results to a CSV file the quizmaster picks; false when they cancel. */
  exportResults: (rows: string[][], separator: ListSeparator, picker: Picker) => invoke<boolean>("export_results", { rows, separator, picker }),
  /** Opens the system's print dialog for this window, which prints what its print style shows. */
  printWindow: () => invoke<void>("print_window"),
  getQuiz: () => invoke<Quiz | null>("get_quiz"),
  /** Folder of the running quiz's media, ending with a path separator. */
  mediaRoot: () => invoke<string | null>("media_root"),
  /** Why the running show's journal cannot be written, if so. */
  showNotSaved: () => invoke<AppError | null>("show_not_saved"),
  /** Free space in bytes on the disk the shows are saved to; null when the system does not say. */
  showDiskSpace: () => invoke<number | null>("show_disk_space"),
  // The backend opens the file pickers: a window names no file to write.
  /** Starts a show of a quiz file the quizmaster picks; false when they cancel. */
  startShowFromFile: (picker: Picker) => invoke<boolean>("start_show_from_file", { picker }),
  /** Starts a show of `recent`, a file in `recentFiles`. */
  startShowFromRecent: (recent: string) => invoke<void>("start_show_from_recent", { recent }),
  /** Quiz files opened, saved or played lately, latest first. */
  recentFiles: () => invoke<string[]>("recent_files"),
  startDemoShow: () => invoke<void>("start_demo_show"),
  lastSession: () => invoke<SessionSummary | null>("last_session"),
  /** Resumes the last show; returns warnings when it runs but not quite as before (its media are missing, other rules). */
  resumeLastSession: () => invoke<AppError[]>("resume_last_session"),
  endShow: () => invoke<void>("end_show"),
  /** `seen`: the step the window shows, for a command that moves the show on (`StepGuard`). */
  dispatch: (command: Command, seen: number | null = null) => invoke<void>("dispatch", { command, seen }),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  /** Opens the projector on the monitor with this `MonitorInfo.id`, or on one the Director is not on (null). */
  openHostWindow: (monitor: string | null, fullscreen: boolean) =>
    invoke<void>("open_host_window", { monitor, fullscreen }),
  closeHostWindow: () => invoke<void>("close_host_window"),
  hostWindowOpen: () => invoke<boolean>("host_window_open"),
  /** Where the projector window is; null while it is closed. */
  projectorWindow: () => invoke<ProjectorWindow | null>("projector_window"),
  reportHostProblem: (message: string) => invoke<void>("report_host_problem", { message }),
  /** Ends the app; the running show stays saved for resuming. */
  quitApp: () => invoke<void>("quit_app"),
  /** The quizmaster chose not to close the app during a show: the next close asks again. */
  closeDeclined: () => invoke<void>("close_declined"),
  /** Notes in the app's log a problem this window has nowhere to show. */
  logProblem: (message: string) => invoke<void>("log_problem", { message }),
  /** Words the macOS app menu in the UI's language; other systems have none. */
  setMenuWords: (words: MenuWords) => invoke<void>("set_menu_words", { words }),
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
  /** Copies files the quizmaster picks into the library; returns their references in order. */
  mediaAdd: (picker: Picker) => invoke<string[]>("media_add", { picker }),
  /** Copies a file dropped into the window into the library; returns its reference. */
  mediaAddDropped: async (file: File) =>
    invoke<string>("media_add_dropped", new Uint8Array(await file.arrayBuffer()), { headers: { [DROPPED_NAME_HEADER]: encodeURIComponent(file.name) } }),
  /** Removes a file from the library and deletes its copy. */
  mediaRemove: (reference: string) => invoke<void>("media_remove", { reference }),
  soundBoard: () => invoke<SoundBoard>("sound_board"),
  soundBoardChange: (change: BoardChange) => invoke<void>("sound_board_change", { change }),
  /** What the media player plays, if anything. */
  mediaNow: () => invoke<MediaNow>("media_now"),
  mediaControl: (command: PlayerCommand) => invoke<void>("media_control", { command }),
  /** The projector's video `id` played to its end. */
  mediaEnded: (id: number) => invoke<void>("media_ended", { id }),
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
  /** Makes a key of the quizmaster's remote do `action` from now on. */
  receiverSetRemoteAction: (key: RemoteKey, action: RemoteAction) => invoke<void>("receiver_set_remote_action", { key, action }),
  /** Gives every key of the remote its default action again. */
  receiverResetRemote: () => invoke<void>("receiver_reset_remote"),
  /** Writes the receiver's recent reports to a file the quizmaster picks; false when they cancel. */
  receiverExportLog: (picker: Picker) => invoke<boolean>("receiver_export_log", { picker }),
  receiverUdevRule: () => invoke<string>("receiver_udev_rule"),
  /** The venue's lights setup, how its output does, and why a kept setup was not used. */
  lightsInfo: () => invoke<LightsInfo>("lights_info"),
  /** Checks and keeps the lights setup, then the lights take it. */
  lightsStore: (setup: LightsSetup) => invoke<void>("lights_store", { setup }),
  /** The USB-DMX interfaces and MIDI outputs on this computer. */
  lightsDevices: () => invoke<LightsDevices>("lights_devices"),
  /** The fixture models of the built-in library whose names hold every word of `query`. */
  lightsLibrary: (query: string) => invoke<LibraryMatches>("lights_library", { query }),
  lightsLibraryInfo: () => invoke<LibraryInfo>("lights_library_info"),
  lightsLibraryUpdate: () => invoke<LibraryInfo>("lights_library_update"),
  lightsLibraryReset: () => invoke<LibraryInfo>("lights_library_reset"),
  /** Holds a look whatever the show does, or lets go with null. */
  lightsHold: (look: number | null) => invoke<void>("lights_hold", { look }),
  lightsBlackout: (on: boolean) => invoke<void>("lights_blackout", { on }),
  /** The 512 channel values the lights show now. */
  lightsUniverse: () => invoke<number[]>("lights_universe"),
  // The editor's commands on a draft name it (`EditorDraft.id`): one that
  // another draft has replaced since is refused with `draftChanged`.
  /** The quiz editor's draft left from last time, if any. */
  editorLoad: () => invoke<EditorDraft | null>("editor_load"),
  editorNew: (title: string) => invoke<EditorDraft>("editor_new", { title }),
  /** Opens `recent`, a file in `recentFiles`, or a quiz file the quizmaster picks; null when they cancel. */
  editorOpen: (recent: string | null, picker: Picker) => invoke<EditorDraft | null>("editor_open", { recent, picker }),
  /** Stores the edited quiz; returns what keeps it from being played. */
  editorUpdate: (draft: string, quiz: Quiz, unsaved: boolean) => invoke<ValidationError[]>("editor_update", { draft, quiz, unsaved }),
  /** Saves over the quiz file the draft came from, unless `saveAs`, else where the quizmaster picks; returns the path, null when they cancel. */
  editorSave: (draft: string, quiz: Quiz, saveAs: boolean, picker: Picker) => invoke<string | null>("editor_save", { draft, quiz, saveAs, picker }),
  /** Copies a media file of `kinds` the quizmaster picks into the draft; returns its reference (`media/...`), null when they cancel. */
  editorAddMedia: (draft: string, kinds: MediaKind[], picker: Picker) => invoke<string | null>("editor_add_media", { draft, kinds, picker }),
  /** A name for a new category, "`base` 2" or so, that none of `categories` has. */
  editorCategoryName: (categories: readonly Category[], base: string) => invoke<string>("editor_category_name", { categories, base }),
  /**
   * Reads questions from a spreadsheet, copying their media into the draft,
   * for the editor to insert; a row's `CAT` names one of the draft's categories.
   */
  editorImportSheet: (draft: string, picker: Picker) => invoke<SheetImport | null>("editor_import_sheet", { draft, picker }),
  /** Writes the quiz as a CSV file the quizmaster picks, with its media next to it; null when they cancel. */
  editorExportSheet: (draft: string, quiz: Quiz, separator: ListSeparator, picker: Picker) =>
    invoke<SheetExport | null>("editor_export_sheet", { draft, quiz, separator, picker }),
  /** Discards the draft, also one that cannot be read. */
  editorClose: () => invoke<void>("editor_close"),
  /** Plays the editor's quiz as a show that is not kept for resuming, from the slide `from` if given. */
  editorTestRun: (draft: string, quiz: Quiz, from: number | null = null) => invoke<void>("editor_test_run", { draft, quiz, from }),
  /** Writes the skill package for chat assistants where the quizmaster picks; false when they cancel. */
  aiSaveSkill: (picker: Picker) => invoke<boolean>("ai_save_skill", { picker }),
  /**
   * Adds the app to Claude Desktop as the MCP server it writes quizzes with, working in the Buzzdome folder of the
   * quizmaster's documents, or with `choose` in a folder they pick; says what changed, null when they cancel.
   */
  aiConnectClaude: (choose: boolean) => invoke<ClaudeConnected | null>("ai_connect_claude", { choose }),
};

/** Events the backend emits (`app.emit` in app/src-tauri), with their payloads. */
export interface BackendEvents {
  /** The game after every accepted command; null when no show runs. */
  "game-state": GameView | null;
  /** A different show (or none) is loaded: reload the quiz. */
  "show-changed": null;
  /** The running question clock, a few times per second. */
  "clock-tick": ClockView;
  /** The running show's journal refused a write (why), or works again (null). */
  "show-not-saved": AppError | null;
  "sound-cue": Cue;
  "receiver-status": ReceiverStatus;
  "receiver-raw": RawReport;
  "receiver-press": PressEvent;
  /** A buzzer the receiver heard, as it now stands. */
  "receiver-buzzer": BuzzerSeen;
  "receiver-learn": LearnProgress;
  /** A key of the quizmaster's remote was pressed, with what it does: the Director does it. */
  "receiver-remote": RemoteKeyAction;
  /** The stage lights' output, look, hold and blackout, whenever one changes. */
  "lights-status": LightsStatus;
  /** The projector window opened (true) or closed (false). */
  "host-window": boolean;
  /** A problem the projector met, worded for the Director to show. */
  "host-problem": string;
  /** The Director was asked to close during a show: the quizmaster confirms it (`quitApp`) or not (`closeDeclined`). */
  "close-requested": null;
  /** The prepared team names changed, or a show started or ended. */
  "teams-changed": string[];
  /** Files came into the media library or left it. */
  "media-library": MediaLibrary;
  "sound-board": SoundBoard;
  /** The media player started, paused or stopped something. */
  "media-playing": MediaNow;
  /** The player's volume changed, in percent. */
  "media-volume": number;
  /** The windows' icon took another colour. */
  "app-icon": IconColor;
}
