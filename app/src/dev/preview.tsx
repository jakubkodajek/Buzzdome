// Dev-only preview: renders a window with a mocked backend so screens can be
// checked in a normal browser. Open /preview.html?window=host&scene=running.

import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import React from "react";
import ReactDOM from "react-dom/client";
import demoQuiz from "../../../samples/demo-quiz.json";
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
    buzzed: null,
    leaderboardVisible: false,
    ...partial,
  };
}

const answered: Partial<PlayerView>[] = [
  { answered: true, answer: "b", answerMs: 3200 },
  { answered: true, answer: "b", answerMs: 4100 },
  { answered: true, answer: "a", answerMs: 5200 },
  {},
  { answered: true, answer: "c", answerMs: 7900 },
];

const SCENES: Record<string, GameView | null> = {
  none: null,
  lobby: view({ phase: { phase: "lobby" }, slide: null, players: players().slice(0, 5) }),
  billboard: view({ phase: { phase: "billboard" }, slideIndex: 0, slide: quiz.slides[0] }),
  ready: view({}),
  running: view({
    phase: { phase: "question", step: "running", clock: { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0 } },
    clock: { limitMs: 20000, remainingMs: 12400, elapsedMs: 7600, running: true },
    players: players(answered),
    answeredCount: 4,
  }),
  closed: view({
    phase: { phase: "question", step: "closed" },
    players: players(answered),
    answeredCount: 4,
    answerCounts: [1, 2, 1, 0],
  }),
  revealed: view({
    phase: { phase: "question", step: "revealed" },
    players: players(answered.map((a, i) => ({ ...a, delta: [84, 80, 0, 0, 0][i] || null }))),
    answeredCount: 4,
    answerCounts: [1, 2, 1, 0],
  }),
  buzzed: view({
    phase: { phase: "question", step: "buzzed", buzzer: 3, clock: { limitMs: 30000, elapsedBeforeMs: 4300, runningSinceMs: null } },
    slideIndex: 6,
    slide: quiz.slides[6],
    clock: { limitMs: 30000, remainingMs: 25700, elapsedMs: 4300, running: false },
    buzzed: 3,
  }),
  ffRevealed: view({
    phase: { phase: "question", step: "revealed" },
    slideIndex: 6,
    slide: quiz.slides[6],
    players: players([{}, {}, { delta: 129 }]),
  }),
  leaderboard: view({ phase: { phase: "endOfRound" }, slideIndex: 4, slide: quiz.slides[4] }),
  overlay: view({ leaderboardVisible: true }),
  final3: view({ phase: { phase: "final", step: 3 }, slide: null }),
};

clearMocks();
mockWindows(windowLabel, windowLabel === "host" ? "director" : "host");
mockIPC((cmd) => {
  switch (cmd) {
    case "get_view":
      return SCENES[scene] ?? null;
    case "get_quiz":
      return quiz;
    case "list_monitors":
      return [
        { index: 0, name: "Built-in Display", width: 2560, height: 1600 },
        { index: 1, name: "Projector", width: 1920, height: 1080 },
      ];
    case "host_window_open":
      return false;
    case "last_session":
      return { meta: { quizTitle: quiz.title, sourcePath: null, startedMs: Date.now() - 3_600_000 }, commandCount: 57 };
    case "plugin:event|listen":
      return 1;
    default:
      return null;
  }
});

const { default: App } = await import("../App");
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
