// Dev-only preview: renders a window with a mocked backend so screens can be
// checked in a normal browser. Open /preview.html?window=host&scene=running
// (add &theme=builtin.neon for another look).

import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import React from "react";
import ReactDOM from "react-dom/client";
import demoQuiz from "../../../samples/demo-quiz.json";
import builtinThemes from "../bindings/BuiltinThemes.json";
import { NO_CONTROLS } from "../host/preview";
import type { Background, GameView, PlayerView, Quiz, SoundClip, VideoClip } from "../types";
import "../styles.css";

// The backend fills serde defaults; the raw JSON file leaves them out.
const quiz = {
  ...demoQuiz,
  slides: demoQuiz.slides.map((slide) => ({
    notes: "",
    answers: [],
    correct: null,
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
};

clearMocks();
mockWindows(windowLabel, windowLabel === "host" ? "director" : "host");
/** The commands the windows sent, for browser checks of the controls (nothing applies them here). */
const dispatched: unknown[] = [];
(window as unknown as { dispatched: unknown[] }).dispatched = dispatched;

mockIPC((cmd, payload) => {
  switch (cmd) {
    case "dispatch":
      dispatched.push((payload as { command: unknown }).command);
      return null;
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
    case "media_root":
      // Served by Vite from this folder (see the convertFileSrc mock below).
      return "/src/dev/";
    case "last_session":
      return { meta: { quizTitle: quiz.title, sourcePath: null, startedMs: Date.now() - 3_600_000 }, commandCount: 57 };
    case "plugin:event|listen":
      return 1;
    case "editor_load":
      return { quiz: editorQuiz, sourcePath: "C:\\Kvizy\\hospodsky-kviz.quiz", unsaved: false, mediaRoot: "/src/dev/", problems: [] };
    case "editor_update":
      return [];
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
});

// The real app loads media through Tauri's asset protocol; here Vite serves them.
(window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (path: string) => string } }).__TAURI_INTERNALS__.convertFileSrc = (path) => path;

const { default: App } = await import("../App");
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
