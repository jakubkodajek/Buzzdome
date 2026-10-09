// Dev-only preview: renders a window with a mocked backend so screens can be
// checked in a normal browser. Open /preview.html?window=host&scene=running.

import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import React from "react";
import ReactDOM from "react-dom/client";
import demoQuiz from "../../../samples/demo-quiz.json";
import { NO_CONTROLS } from "../host/preview";
import type { GameView, PlayerView, Quiz } from "../types";
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
    enabledCount: 8,
    answeredCount: 0,
    answerCounts: null,
    results: null,
    buzzed: null,
    leaderboardVisible: false,
    controls: { ...NO_CONTROLS, next: true },
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
  lobby: view({ phase: { phase: "lobby" }, slide: null, players: players().slice(0, 5) }),
  billboard: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: quiz.slides[0] }),
  ready: view({}),
  running: view({
    phase: { phase: "question", step: "running", clock: { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0 } },
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
    phase: { phase: "question", step: "buzzed", buzzer: 3, clock: { limitMs: 30000, elapsedBeforeMs: 4300, runningSinceMs: null } },
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
  explanation: view({ phase: { phase: "explanation" }, slideIndex: 4, slide: quiz.slides[4] }),
  leaderboard: view({ phase: { phase: "endOfRound" }, slideIndex: 5, slide: quiz.slides[5] }),
  overlay: view({ leaderboardVisible: true }),
  final3: view({ phase: { phase: "final", step: 3 }, slide: null }),
  picture: view({
    phase: { phase: "question", step: "ready" },
    slide: { ...quiz.slides[1], picture: "media/landscape.svg", sound: { file: "media/song.mp3", start: "onDemand", repeat: false, volume: 80 } },
  }),
  billboardPicture: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: { ...quiz.slides[0], picture: "media/landscape.svg" } }),
};

// The quiz as the scene's view shows it (scenes may add media to a slide).
const current = SCENES[scene] ?? null;
const sceneQuiz: Quiz = {
  ...quiz,
  slides: quiz.slides.map((slide, i) => (current?.slide && i === current.slideIndex ? current.slide : slide)),
};

// The quiz the editor opens: the demo quiz, with media on its first question.
const editorQuiz: Quiz = {
  ...quiz,
  slides: quiz.slides.map((slide, i) =>
    i === 1 ? { ...slide, picture: "media/landscape.svg", sound: { file: "media/song.mp3", start: "onDemand", repeat: false, volume: 80 } } : slide,
  ),
};

clearMocks();
mockWindows(windowLabel, windowLabel === "host" ? "director" : "host");
mockIPC((cmd) => {
  switch (cmd) {
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
