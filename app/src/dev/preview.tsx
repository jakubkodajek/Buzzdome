// Dev-only preview: renders a window with a mocked backend so screens can be
// checked in a normal browser. Open /preview.html?window=host&scene=running
// (add &theme=builtin.neon for another look).

import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import React from "react";
import ReactDOM from "react-dom/client";
import demoQuiz from "../../../samples/demo-quiz.json";
import builtinThemes from "../bindings/BuiltinThemes.json";
import { setBodyText } from "../editor/slides";
import { NO_CONTROLS } from "../host/preview";
import { MAX_BOARD_COLUMNS, MAX_BOARD_ROWS } from "../bindings/Constants";
import type {
  Background,
  BoardChange,
  GameView,
  MediaLibrary,
  NowPlaying,
  Pad,
  PlayerCommand,
  PlayerView,
  Quiz,
  SheetImport,
  SlideStyle,
  SoundBoard,
  SoundClip,
  VideoClip,
} from "../types";
import "../styles.css";

// The backend fills serde defaults; the raw JSON file leaves them out.
const quiz = {
  ...demoQuiz,
  slides: demoQuiz.slides.map((slide) => ({
    notes: "",
    answers: [],
    correct: [],
    pointsEnd: null,
    penalty: 0,
    picture: null,
    video: null,
    sound: null,
    background: null,
    text: "",
    ...slide,
  })),
} as unknown as Quiz;
const params = new URLSearchParams(location.search);
const windowLabel = params.get("window") === "host" ? "host" : "director";
const scene = params.get("scene") ?? "running";
/** What the media player plays at first: `picture` or `video` (for the projector's overlay). */
const playing = params.get("media");
/** A theme id to draw the screens in; the default when missing. */
const theme = params.get("theme");

const NAMES = ["Kvízožrouti", "U Tří Lvů", "Mozkovna", "Hospodští géniové", "Tým Rychlík", "Bezejmenní", "Šprti", "Lvíčata"];

function players(overrides: Partial<PlayerView>[] = []): PlayerView[] {
  const list = NAMES.map((name, i) => ({
    buzzer: i + 1,
    name,
    score: [420, 395, 395, 310, 280, 150, 95, 40][i],
    enabled: true,
    totalAnswerMs: 30_000 + i * 1_700,
    rank: i + 1,
    answered: false,
    answer: null,
    answerCorrect: null,
    answerMs: null,
    lockedOut: false,
    delta: null,
    ...overrides[i],
  }));
  return list;
}

/** A song for the scenes with media: played on demand, its chorus only. */
const SONG: SoundClip = {
  file: "media/song.mp3",
  start: "onDemand",
  repeat: false,
  volume: 80,
  part: { fromMs: 15_000, toMs: 45_000 },
  gainDb: -2.5,
  replay: { part: { fromMs: 45_000, toMs: null }, delayMs: 1_000 },
};

/**
 * A video for the scenes with one (a file generated locally, see
 * .gitignore): its part from 0:05 to 0:20 at 150 %, under a mosaic, and
 * from 0:25 again after the reveal.
 */
const VIDEO: VideoClip = {
  file: "media/clip.webm",
  autoplay: true,
  introPauseMs: 0,
  muted: false,
  repeat: false,
  volume: 60,
  speed: 150,
  part: { fromMs: 5_000, toMs: 20_000 },
  effect: "mosaic",
  replay: { part: { fromMs: 25_000, toMs: null }, delayMs: 0 },
};

/** A slide's own look: a gradient behind it, a left-aligned serif headline, see-through answers. */
const STYLE: SlideStyle = {
  fill: { from: "#0B1E3F", to: "#3A6EA5CC", kind: "diagonal" },
  headline: { color: "#FFCC00", font: "Georgia", size: 80, italic: true, align: "left", shadowColor: "#FF000080" },
  text: { underline: true, fill: { from: "#00000099", to: "#00000033", kind: "horizontal" } },
  answers: { fill: { from: "#FFFFFF", to: "#B8C8D8", kind: "vertical" }, color: "#102030", transparency: 20, align: "left", bold: false },
};

/** A picture behind a slide, with the options a new background gets. */
const BACKGROUND: Background = { file: "media/landscape.svg", repeat: true, muted: true, sync: false };

function view(partial: Partial<GameView>): GameView {
  return {
    quizTitle: quiz.title,
    slideIndex: 1,
    slideCount: quiz.slides.length,
    phase: { phase: "question", step: "ready" },
    slide: quiz.slides[1],
    nextSlideTitle: "Která řeka protéká Českými Budějovicemi?",
    clock: null,
    players: players(),
    teams: [],
    enabledCount: 8,
    answeredCount: 0,
    answerCounts: null,
    results: null,
    buzzed: null,
    leaderboardVisible: false,
    controls: { ...NO_CONTROLS, next: true },
    onHold: false,
    videoPlays: 0,
    ...partial,
  };
}

const answered: Partial<PlayerView>[] = [
  { answered: true, answer: "b", answerCorrect: true, answerMs: 3200 },
  { answered: true, answer: "b", answerCorrect: true, answerMs: 4100 },
  { answered: true, answer: "a", answerCorrect: false, answerMs: 5200 },
  {},
  { answered: true, answer: "c", answerCorrect: false, answerMs: 7900 },
];

const SCENES: Record<string, GameView | null> = {
  none: null,
  // Two buzzers paired with prepared teams, three still without a team.
  lobby: view({
    phase: { phase: "lobby" },
    slide: null,
    players: players([{}, {}, { name: "" }, { name: "" }, { name: "" }]).slice(0, 5),
    teams: [
      { name: NAMES[0], buzzer: 1 },
      { name: "Pivní rytíři", buzzer: null },
      { name: NAMES[1], buzzer: 2 },
      { name: "Sovy", buzzer: null },
    ],
  }),
  billboard: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: quiz.slides[0] }),
  ready: view({}),
  running: view({
    phase: { phase: "question", step: "running", clock: { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 } },
    clock: { limitMs: 20000, remainingMs: 12400, elapsedMs: 7600, running: true, points: 81 },
    players: players(answered),
    answeredCount: 4,
    controls: { ...NO_CONTROLS, next: true, pauseClock: true, closeQuestion: true },
  }),
  closed: view({
    phase: { phase: "question", step: "closed" },
    players: players(answered),
    answeredCount: 4,
    answerCounts: [1, 2, 1, 0],
    results: { right: 2, wrong: 2, none: 1 },
  }),
  revealed: view({
    phase: { phase: "question", step: "revealed", winner: null },
    results: { right: 2, wrong: 2, none: 1 },
    players: players(answered.map((a, i) => ({ ...a, delta: [84, 80, 0, 0, 0][i] || null }))),
    answeredCount: 4,
    answerCounts: [1, 2, 1, 0],
  }),
  buzzed: view({
    phase: { phase: "question", step: "buzzed", buzzer: 3, clock: { limitMs: 30000, elapsedBeforeMs: 4300, runningSinceMs: null, pressesFromMs: 0 } },
    slideIndex: 7,
    slide: quiz.slides[7],
    clock: { limitMs: 30000, remainingMs: 25700, elapsedMs: 4300, running: false, points: 129 },
    buzzed: 3,
    controls: { ...NO_CONTROLS, judge: true },
  }),
  ffRevealed: view({
    phase: { phase: "question", step: "revealed", winner: 3 },
    slideIndex: 7,
    slide: quiz.slides[7],
    players: players([{}, {}, { delta: 129 }]),
  }),
  sixAnswers: view({
    phase: { phase: "question", step: "revealed", winner: 3 },
    slideIndex: 8,
    slide: quiz.slides[8],
    players: players([{}, {}, { delta: 150 }]),
  }),
  sixAnswersPicture: view({ slideIndex: 8, slide: { ...quiz.slides[8], picture: "media/landscape.svg" } }),
  explanation: view({ phase: { phase: "explanation" }, slideIndex: 4, slide: quiz.slides[4] }),
  leaderboard: view({ phase: { phase: "endOfRound" }, slideIndex: 5, slide: quiz.slides[5] }),
  overlay: view({ leaderboardVisible: true }),
  final3: view({ phase: { phase: "final", step: 3 }, slideIndex: quiz.slides.length, slide: null }),
  // The demo quiz's final slide, with the winners' music to play on demand.
  finalSlide: view({ phase: { phase: "final", step: 3 }, slideIndex: quiz.slides.length - 1, slide: { ...quiz.slides[quiz.slides.length - 1], sound: SONG } }),
  picture: view({
    phase: { phase: "question", step: "ready" },
    slide: { ...quiz.slides[1], picture: "media/landscape.svg", sound: SONG },
  }),
  // The same question revealed: the song's replay part plays after its delay.
  pictureRevealed: view({
    phase: { phase: "question", step: "revealed", winner: null },
    slide: { ...quiz.slides[1], picture: "media/landscape.svg", sound: SONG },
  }),
  billboardPicture: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: { ...quiz.slides[0], picture: "media/landscape.svg" } }),
  background: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: { ...quiz.slides[0], background: BACKGROUND } }),
  // A question's video in each step, with its effect fading over the answer time.
  videoReady: view({ slideIndex: 2, slide: { ...quiz.slides[2], video: VIDEO } }),
  videoRunning: view({
    slideIndex: 2,
    slide: { ...quiz.slides[2], video: VIDEO },
    phase: { phase: "question", step: "running", clock: { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 } },
    clock: { limitMs: 20000, remainingMs: 12000, elapsedMs: 8000, running: true, points: 81 },
  }),
  videoBlur: view({
    slideIndex: 2,
    slide: { ...quiz.slides[2], video: { ...VIDEO, effect: "blur" } },
    phase: { phase: "question", step: "running", clock: { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 } },
    clock: { limitMs: 20000, remainingMs: 10000, elapsedMs: 10000, running: false, points: 81 },
  }),
  videoClosed: view({ slideIndex: 2, slide: { ...quiz.slides[2], video: VIDEO }, phase: { phase: "question", step: "closed" }, onHold: true }),
  videoRevealed: view({ slideIndex: 2, slide: { ...quiz.slides[2], video: VIDEO }, phase: { phase: "question", step: "revealed", winner: null }, onHold: true }),
  // A video that waits for the quizmaster, before and after Play video.
  videoWaiting: view({
    phase: { phase: "billboard" },
    slideIndex: 0,
    slide: { ...quiz.slides[0], video: { ...VIDEO, autoplay: false, effect: null } },
    controls: { ...NO_CONTROLS, next: true, playVideo: true },
  }),
  videoPlayed: view({
    phase: { phase: "billboard" },
    slideIndex: 0,
    slide: { ...quiz.slides[0], video: { ...VIDEO, autoplay: false, effect: null } },
    controls: { ...NO_CONTROLS, next: true, playVideo: true },
    videoPlays: 1,
  }),
  brokenBackground: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: { ...quiz.slides[0], background: { ...BACKGROUND, file: "media/missing.png" } } }),
  styled: view({ slide: { ...quiz.slides[1], style: STYLE } }),
  styledBillboard: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: { ...setBodyText(quiz.slides[0], "Zeměpis a historie"), style: STYLE } }),
};

// The quiz as the scene's view shows it (scenes may add media to a slide).
const current = SCENES[scene] ?? null;
const sceneQuiz: Quiz = {
  ...quiz,
  theme,
  slides: quiz.slides.map((slide, i) => (current?.slide && i === current.slideIndex ? current.slide : slide)),
};

// The quiz the editor opens: the demo quiz, with media on its first two questions.
const editorQuiz: Quiz = {
  ...quiz,
  theme,
  slides: quiz.slides.map((slide, i) =>
    i === 1 ? { ...slide, picture: "media/landscape.svg", sound: SONG } : i === 2 ? { ...slide, video: VIDEO } : slide,
  ),
  // A category with a looping background video, for the Categories view.
  categories: [
    {
      id: "c-films",
      name: "Filmy",
      background: { file: "media/clip.webm", repeat: true, muted: true, sync: false },
      style: { headline: { color: "#ffd54a", bold: true }, answers: { fill: { from: "#3a1c71", to: "#d76d77", kind: "horizontal" } } },
    },
  ],
};

clearMocks();
mockWindows(windowLabel, windowLabel === "host" ? "director" : "host");
/** The commands the windows sent, for browser checks of the controls (nothing applies them here). */
const dispatched: unknown[] = [];
(window as unknown as { dispatched: unknown[] }).dispatched = dispatched;
/** What the stand-in spreadsheet import gives. */
const SHEET_IMPORT: SheetImport = {
  slides: [
    { id: "import-1", notes: "", picture: null, video: null, sound: null, background: null, type: "billboard", title: "Kolo 2", text: "Filmy" },
    {
      id: "import-2",
      notes: "",
      picture: null,
      video: null,
      sound: null,
      background: null,
      type: "question",
      category: "c-films",
      text: "Kdo natočil Vetřelce?",
      mode: "everyone",
      answers: ["James Cameron", "Ridley Scott", "David Fincher"],
      correct: [{ answer: 1, share: 100 }],
      timeLimitS: 20,
      points: 10,
      pointsEnd: null,
      penalty: 0,
    },
  ],
  notes: [
    { issue: { code: "unknownCategory", name: "Hudba" }, places: [{ sheet: "Kolo 2", row: 1 }] },
    { issue: { code: "fileNotFound", file: "ridley.jpg" }, places: [{ sheet: "Kolo 2", row: 3 }] },
  ],
};

/** Other backend calls that change something, as `{ cmd, ...payload }`, for the same checks. */
const called: unknown[] = [];
(window as unknown as { called: unknown[] }).called = called;
/** The team names prepared before a show (the running show's in a show scene). */
const PREPARED_TEAMS = ["Kvízožrouti", "Pivní rytíři", "U Tří Lvů"];

// The media player's library (files generated locally, see .gitignore) and a board a quizmaster arranged.
const LIBRARY: MediaLibrary = {
  root: "/src/dev/",
  files: [
    { reference: "media/applause.wav", kind: "audio" },
    { reference: "media/clip.webm", kind: "video" },
    { reference: "media/jingle.wav", kind: "audio" },
    { reference: "media/landscape.svg", kind: "image" },
  ],
};
const filePad = (reference: string, label = "", color: Pad["color"] = null): Pad => ({ source: { type: "file", reference }, label, color });
let board: SoundBoard = {
  rows: 3,
  columns: 4,
  click: "stop",
  arranged: true,
  pads: Array.from({ length: MAX_BOARD_ROWS * MAX_BOARD_COLUMNS }, (_, place): Pad => {
    const pads: Record<number, Pad> = {
      0: filePad("media/jingle.wav"),
      1: filePad("media/applause.wav", "Potlesk", "green"),
      2: filePad("media/clip.webm"),
      3: filePad("media/landscape.svg"),
      [MAX_BOARD_COLUMNS]: { source: { type: "slideSound" }, label: "", color: null },
      [MAX_BOARD_COLUMNS + 1]: { source: { type: "slideVideo" }, label: "", color: null },
    };
    return pads[place] ?? { source: null, label: "", color: null };
  }),
};
let nowPlaying: NowPlaying | null =
  playing === "picture" ? { id: 1, reference: "media/landscape.svg", kind: "image", positionMs: 0, sinceMs: Date.now() }
  : playing === "video" ? { id: 1, reference: "media/clip.webm", kind: "video", positionMs: 3_000, sinceMs: Date.now() }
  : null;

/** A rough stand-in for the backend's media player (`quiz_core::MediaPlayer`), enough to click through the tab. */
function control(command: PlayerCommand) {
  const at = Date.now();
  const start = (reference: string) => {
    const kind = LIBRARY.files.find((file) => file.reference === reference)?.kind ?? "audio";
    nowPlaying = { id: (nowPlaying?.id ?? 0) + 1, reference, kind, positionMs: 0, sinceMs: at };
  };
  const position = (now: NowPlaying) => now.positionMs + (now.sinceMs === null ? 0 : at - now.sinceMs);
  switch (command.type) {
    case "play":
      if (nowPlaying?.reference === command.reference && nowPlaying.sinceMs === null) nowPlaying = { ...nowPlaying, sinceMs: at };
      else start(command.reference);
      break;
    case "pad": {
      const source = board.pads[command.pad].source;
      if (source?.type !== "file") break;
      if (nowPlaying?.reference !== source.reference) start(source.reference);
      else if (nowPlaying.sinceMs === null) nowPlaying = { ...nowPlaying, sinceMs: at };
      else if (board.click === "stop") nowPlaying = null;
      else if (board.click === "restart") start(source.reference);
      else nowPlaying = { ...nowPlaying, positionMs: position(nowPlaying), sinceMs: null };
      break;
    }
    case "pause":
      if (nowPlaying) nowPlaying = { ...nowPlaying, positionMs: position(nowPlaying), sinceMs: null };
      break;
    case "resume":
      if (nowPlaying?.sinceMs === null) nowPlaying = { ...nowPlaying, sinceMs: at };
      break;
    case "rewind":
      if (nowPlaying) nowPlaying = { ...nowPlaying, positionMs: 0, sinceMs: nowPlaying.sinceMs === null ? null : at };
      break;
    case "stop":
      nowPlaying = null;
      break;
    case "ended":
      if (nowPlaying?.id === command.id) nowPlaying = null;
      break;
  }
  void emit("media-playing", nowPlaying);
}

/** The same for the board's changes the preview checks. */
function changeBoard(change: BoardChange) {
  const pads = [...board.pads];
  switch (change.type) {
    case "resize":
      board = { ...board, rows: change.rows, columns: change.columns };
      break;
    case "setClick":
      board = { ...board, click: change.click };
      break;
    case "swap":
      [pads[change.a], pads[change.b]] = [pads[change.b], pads[change.a]];
      board = { ...board, pads, arranged: true };
      break;
    case "setLabel":
      pads[change.pad] = { ...pads[change.pad], label: change.label };
      board = { ...board, pads, arranged: true };
      break;
    case "setColor":
      pads[change.pad] = { ...pads[change.pad], color: change.color };
      board = { ...board, pads, arranged: true };
      break;
    case "setSource":
      pads[change.pad] = { ...pads[change.pad], source: change.source };
      board = { ...board, pads, arranged: true };
      break;
    case "place":
      pads[change.pad] = { ...pads[change.pad], source: { type: "file", reference: change.references[0] } };
      board = { ...board, pads, arranged: true };
      break;
    case "clearPad":
      pads[change.pad] = { source: null, label: "", color: null };
      board = { ...board, pads, arranged: true };
      break;
    default:
      break;
  }
  void emit("sound-board", board);
}

mockIPC(
  (cmd, payload) => {
    switch (cmd) {
      case "dispatch":
        dispatched.push((payload as { command: unknown }).command);
        return null;
      case "add_teams":
      case "remove_team":
      case "media_add":
      case "media_remove":
        called.push({ cmd, ...(payload as object) });
        return null;
      case "media_library":
        return LIBRARY;
      case "sound_board":
        return board;
      case "sound_board_change":
        called.push({ cmd, ...(payload as object) });
        changeBoard((payload as { change: BoardChange }).change);
        return null;
      case "media_now":
        return nowPlaying;
      case "media_control":
        called.push({ cmd, ...(payload as object) });
        control((payload as { command: PlayerCommand }).command);
        return null;
      case "prepared_teams":
        return current ? current.teams.map((team) => team.name) : PREPARED_TEAMS;
      case "teams_lost":
        // `?teamsLost` shows the notice for saved names that could not be read.
        return params.has("teamsLost") ? { detail: "expected value at line 1 column 1", keptAs: "C:\\Data\\teams.json.broken" } : null;
      case "get_view":
        return current;
      case "get_quiz":
        return sceneQuiz;
      case "list_monitors":
        return [
          { index: 0, name: "Built-in Display", width: 2560, height: 1600 },
          { index: 1, name: "Projector", width: 1920, height: 1080 },
        ];
      case "host_window_open":
        return false;
      case "list_themes":
        // Their files are served by Vite from src-tauri/themes.
        return builtinThemes;
      case "installed_fonts":
        return ["Arial", "Courier New", "DejaVu Sans", "Georgia", "Impact", "Times New Roman", "Verdana"];
      case "media_root":
        // Served by Vite from this folder (see the convertFileSrc mock below).
        return "/src/dev/";
      case "resume_last_session":
        return [];
      case "last_session":
        return { meta: { quizTitle: quiz.title, sourcePath: null, startedMs: Date.now() - 3_600_000 }, commandCount: 57 };
      case "editor_load":
        return { quiz: editorQuiz, sourcePath: "C:\\Kvizy\\hospodsky-kviz.quiz", unsaved: false, mediaRoot: "/src/dev/", problems: [], dropped: [] };
      case "editor_open": {
        // As a file from an older version opens: what this one leaves out is listed.
        const [first, , third] = editorQuiz.slides;
        const dropped = [
          { slideId: first.id, problem: { code: "styleUnread", path: "style.headline.color" } },
          { slideId: third.id, problem: { code: "mediaOnRanking" } },
        ];
        const path = (payload as { path: string }).path;
        return { quiz: editorQuiz, sourcePath: path, unsaved: true, mediaRoot: "/src/dev/", problems: [], dropped };
      }
      case "editor_update":
        called.push({ cmd, ...(payload as object) });
        return [];
      // Dialogs answer what a check sets in `window.dialogAnswer`; cancelled otherwise.
      case "plugin:dialog|open":
      case "plugin:dialog|save":
        return (window as unknown as { dialogAnswer?: string }).dialogAnswer ?? null;
      case "editor_import_sheet":
        called.push({ cmd, ...(payload as object) });
        return SHEET_IMPORT;
      case "editor_export_sheet":
        called.push({ cmd, ...(payload as object) });
        return { media: 2, missing: ["media/gone.mp3"] };
      case "receiver_info":
        return {
          status: { kind: "error", problem: "cannotOpen", detail: "access denied" },
          initError: null,
          settingsError: null,
          learnedButtons: 0,
          learn: { active: false, buzzer: null, button: null, samples: 0, canSave: false },
          recent: [],
          platform: "linux",
        };
      case "receiver_devices":
        return [];
      case "receiver_udev_rule":
        return 'SUBSYSTEM=="hidraw", ATTRS{idVendor}=="VVVV", MODE="0660", TAG+="uaccess"';
      default:
        return null;
    }
  },
  // Events the mock emits (the media player's) reach the windows' listeners.
  { shouldMockEvents: true },
);

// The real app loads media through Tauri's asset protocol; here Vite serves them.
(window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (path: string) => string } }).__TAURI_INTERNALS__.convertFileSrc = (path) => path;

const { default: App } = await import("../App");
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
