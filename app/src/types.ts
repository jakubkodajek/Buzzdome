// Mirrors of the Rust types in crates/quiz-core, as serialized by serde.

export type BuzzerId = number;
export type Button = "a" | "b" | "c" | "d" | "big";
export type AnswerMode = "everyone" | "fastestFinger";
export type Cue = "clockStart" | "buzz" | "correct" | "wrong" | "timeUp" | "reveal";

export interface Question {
  type: "question";
  text: string;
  mode: AnswerMode;
  answers: string[];
  correct: number | null;
  timeLimitS: number;
  points: number;
  pointsEnd: number | null;
  penalty: number;
  picture: string | null;
}

export type SlideContent =
  | { type: "billboard"; title: string; text: string; picture: string | null }
  | Question
  | { type: "endOfRound"; title: string };

export type Slide = SlideContent & { id: string; notes: string };

export interface Quiz {
  schemaVersion: number;
  title: string;
  slides: Slide[];
}

export interface Clock {
  limitMs: number | null;
  elapsedBeforeMs: number;
  runningSinceMs: number | null;
}

export type QuestionStep =
  | { step: "ready" }
  | { step: "running"; clock: Clock }
  | { step: "buzzed"; buzzer: BuzzerId; clock: Clock }
  | { step: "closed" }
  | { step: "revealed" };

export type Phase =
  | { phase: "lobby" }
  | { phase: "billboard" }
  | { phase: "endOfRound" }
  | ({ phase: "question" } & QuestionStep)
  | { phase: "final"; step: number };

export interface ClockView {
  limitMs: number | null;
  remainingMs: number | null;
  elapsedMs: number;
  running: boolean;
}

export interface PlayerView {
  buzzer: BuzzerId;
  name: string;
  score: number;
  enabled: boolean;
  totalAnswerMs: number;
  rank: number | null;
  answered: boolean;
  /** Director only: never show before the question closes. */
  answer: Button | null;
  answerMs: number | null;
  lockedOut: boolean;
  delta: number | null;
}

export interface GameView {
  quizTitle: string;
  slideIndex: number;
  slideCount: number;
  phase: Phase;
  slide: Slide | null;
  nextSlideTitle: string | null;
  clock: ClockView | null;
  players: PlayerView[];
  enabledCount: number;
  answeredCount: number;
  answerCounts: number[] | null;
  buzzed: BuzzerId | null;
  leaderboardVisible: boolean;
}

export type Command =
  | { type: "press"; buzzer: BuzzerId; button: Button }
  | { type: "next" }
  | { type: "goTo"; slide: number }
  | { type: "pauseClock" }
  | { type: "resumeClock" }
  | { type: "closeQuestion" }
  | { type: "judgeRight" }
  | { type: "judgeWrong" }
  | { type: "addPlayer"; buzzer: BuzzerId; name: string }
  | { type: "renamePlayer"; buzzer: BuzzerId; name: string }
  | { type: "setPlayerEnabled"; buzzer: BuzzerId; enabled: boolean }
  | { type: "adjustScore"; buzzer: BuzzerId; delta: number }
  | { type: "showLeaderboard"; visible: boolean };

export interface SessionSummary {
  meta: { quizTitle: string; sourcePath: string | null; startedMs: number };
  commandCount: number;
}

export interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
}

export const ANSWER_LETTERS = ["A", "B", "C", "D", "E", "F"];
