import { useCallback, useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { questionNumber, useGameView, useQuiz, useRemainingMs, useSoundCues } from "../api";
import { playerName, translator, useLang, type Translate } from "../i18n";
import { useShortcuts } from "../shortcuts";
import { playCue } from "../sound";
import { ANSWER_LETTERS, type Cue, type GameView, type PlayerView, type Question } from "../types";
import { CheckIcon, CrossIcon, PeopleIcon, QuestionIcon, TrophyIcon } from "../icons";
import "./host.css";

const STAGE_W = 1920;
const STAGE_H = 1080;

/** The projector. Renders the game state; owns nothing. */
export function HostScreen() {
  const [lang] = useLang();
  const t = translator(lang);
  const { view, receivedAt } = useGameView();
  const quiz = useQuiz(view);
  const ignore = useCallback(() => {}, []);
  useShortcuts(view, ignore);
  const verdict = useVerdict();

  return (
    <Stage>
      {view && (
        <>
          <HostContent t={t} view={view} receivedAt={receivedAt} questionNo={questionNumber(quiz, view.slideIndex)} />
          {verdict && <div className={`verdict verdict-${verdict}`}>{verdict === "correct" ? <CheckIcon /> : <CrossIcon />}</div>}
          {view.leaderboardVisible && (
            <div className="overlay">
              <Leaderboard t={t} players={ranked(view)} />
            </div>
          )}
        </>
      )}
    </Stage>
  );
}

/** Plays sound cues and flashes a big check or cross after a verdict. */
function useVerdict(): "correct" | "wrong" | null {
  const [verdict, setVerdict] = useState<"correct" | "wrong" | null>(null);
  const onCue = useCallback((cue: Cue) => {
    playCue(cue);
    if (cue === "correct" || cue === "wrong") setVerdict(cue);
  }, []);
  useSoundCues(onCue);
  useEffect(() => {
    if (!verdict) return;
    const timer = setTimeout(() => setVerdict(null), 1600);
    return () => clearTimeout(timer);
  }, [verdict]);
  return verdict;
}

/** A fixed 16:9 canvas scaled to the window, letterboxed. */
function Stage({ children }: { children: ReactNode }) {
  const [scale, setScale] = useState(1);
  useLayoutEffect(() => {
    const fit = () => setScale(Math.min(window.innerWidth / STAGE_W, window.innerHeight / STAGE_H));
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, []);
  return (
    <div className="host-viewport">
      <div className="stage" style={{ width: STAGE_W, height: STAGE_H, transform: `translate(-50%, -50%) scale(${scale})` }}>
        {children}
      </div>
    </div>
  );
}

function ranked(view: GameView): PlayerView[] {
  return view.players.filter((p) => p.rank !== null);
}

function pad3(n: number): string {
  return n < 0 ? `-${String(-n).padStart(2, "0")}` : String(n).padStart(3, "0");
}

interface ContentProps {
  t: Translate;
  view: GameView;
  receivedAt: number;
  questionNo: { n: number; total: number } | null;
}

function HostContent({ t, view, receivedAt, questionNo }: ContentProps) {
  const phase = view.phase;
  switch (phase.phase) {
    case "lobby":
      return <Welcome t={t} view={view} />;
    case "billboard":
      return view.slide?.type === "billboard" ? (
        <div className="center-slide">
          <h1 className="slide-title">{view.slide.title}</h1>
          {view.slide.text && <p className="slide-text">{view.slide.text}</p>}
        </div>
      ) : null;
    case "endOfRound":
      return (
        <div className="center-slide">
          <h1 className="slide-title small">{view.slide?.type === "endOfRound" ? view.slide.title : t("endOfRound")}</h1>
          <Leaderboard t={t} players={ranked(view)} />
        </div>
      );
    case "final":
      return <FinalScores t={t} players={ranked(view)} step={phase.step} />;
    case "question":
      return view.slide?.type === "question" ? (
        <QuestionScreen t={t} view={view} question={view.slide} receivedAt={receivedAt} questionNo={questionNo} />
      ) : null;
  }
}

function Welcome({ t, view }: { t: Translate; view: GameView }) {
  const registered = new Set(view.players.map((p) => p.buzzer));
  const highest = Math.max(20, ...view.players.map((p) => p.buzzer));
  const cells = Array.from({ length: Math.min(highest, 100) }, (_, i) => i + 1);
  return (
    <div className="welcome">
      <header className="welcome-header">{view.quizTitle}</header>
      <div className="welcome-body">
        <p className="slide-text">{t("lobbyHint")}</p>
        <div className="register-grid">
          {cells.map((n) => (
            <span key={n} className={registered.has(n) ? "on" : ""}>
              {n}
            </span>
          ))}
        </div>
      </div>
      <footer className="welcome-footer">{t("registered", { n: view.players.length })}</footer>
    </div>
  );
}

/** Points available right now; mirrors Question::points_at in the core. */
function pointsAt(q: Question, elapsedMs: number): number {
  if (q.pointsEnd === null || q.timeLimitS === 0) return q.points;
  const limit = q.timeLimitS * 1000;
  return Math.round(q.points + ((q.pointsEnd - q.points) * Math.min(elapsedMs, limit)) / limit);
}

function QuestionScreen(props: ContentProps & { question: Question }) {
  const { t, view, question, receivedAt, questionNo } = props;
  const remaining = useRemainingMs(view, receivedAt);
  const phase = view.phase;
  const step = phase.phase === "question" ? phase.step : "ready";
  const closed = step === "closed" || step === "revealed";
  const limitMs = question.timeLimitS * 1000;
  const elapsed = remaining === null ? (view.clock?.elapsedMs ?? 0) : limitMs - remaining;
  const enabled = view.players.filter((p) => p.enabled);
  const waiting = enabled.filter((p) => !p.answered);

  let middle: ReactNode = null;
  if (question.mode === "everyone" && closed && view.answerCounts) {
    const right = question.correct === null ? 0 : view.answerCounts[question.correct];
    const answered = view.answerCounts.reduce((a, b) => a + b, 0);
    middle = <ResultBars right={right} wrong={answered - right} none={enabled.length - answered} total={enabled.length} />;
  } else if (question.mode === "everyone") {
    middle = (
      <div className="responders">
        {waiting.slice(0, 40).map((p) => (
          <span key={p.buzzer}>{p.buzzer}</span>
        ))}
      </div>
    );
  }

  const buzzed = view.players.find((p) => p.buzzer === view.buzzed);
  const winner = step === "revealed" && question.mode === "fastestFinger" ? view.players.find((p) => (p.delta ?? 0) > 0) : undefined;
  const barPlayer = buzzed ?? winner;

  return (
    <div className="question-screen">
      <div className="info-panel">
        <ClockGauge
          seconds={step === "ready" ? question.timeLimitS : closed ? 0 : remaining === null ? null : Math.ceil(remaining / 1000)}
          fraction={closed ? 0 : step === "ready" || limitMs === 0 || remaining === null ? 1 : remaining / limitMs}
          label={questionNo ? `${questionNo.n}/${questionNo.total}` : ""}
        />
        <div className="info-middle">{middle}</div>
        {question.mode === "everyone" && (
          <div className="info-stat">
            <PeopleIcon />
            <span className="digits">{view.answeredCount}</span>
          </div>
        )}
        <div className="info-stat">
          <TrophyIcon />
          <span className="digits">{pointsAt(question, elapsed)}</span>
        </div>
      </div>

      <h1 className="question-text">{question.text}</h1>

      {question.answers.length > 0 && (
        <div className={`answers answers-${question.answers.length > 4 ? "6" : "4"}`}>
          {question.answers.map((answer, i) => {
            const revealed = step === "revealed";
            const isCorrect = i === question.correct;
            return (
              <div key={i} className={["answer", `answer-${i}`, revealed ? (isCorrect ? "is-correct" : "is-dim") : ""].join(" ")}>
                <span className="letter">{ANSWER_LETTERS[i]}</span>
                <span className="answer-text">{answer}</span>
                {closed && view.answerCounts && <span className="answer-count">{view.answerCounts[i]}</span>}
                {revealed && isCorrect && <CheckIcon className="answer-check" />}
              </div>
            );
          })}
        </div>
      )}

      {question.mode === "fastestFinger" && barPlayer && (
        <div className="buzz-bar">
          <span className="buzz-name">{playerName(t, barPlayer)}</span>
          {barPlayer.delta !== null && barPlayer.delta !== 0 && (
            <span className={`buzz-delta ${barPlayer.delta > 0 ? "plus" : "minus"}`}>
              {barPlayer.delta > 0 ? `+${barPlayer.delta}` : barPlayer.delta}
            </span>
          )}
        </div>
      )}
      {question.mode === "fastestFinger" && step === "running" && !barPlayer && (
        <div className="buzz-bar idle">{t("waitingForBuzz")}</div>
      )}
      {step === "closed" && remaining === 0 && question.mode === "fastestFinger" && (
        <div className="buzz-bar idle">{t("timeUp")}</div>
      )}
    </div>
  );
}

function ClockGauge({ seconds, fraction, label }: { seconds: number | null; fraction: number; label: string }) {
  const r = 70;
  const circumference = 2 * Math.PI * r;
  const urgent = seconds !== null && seconds <= 5;
  return (
    <div className={`clock-gauge ${urgent ? "urgent" : ""}`}>
      <svg viewBox="0 0 180 180">
        <circle cx="90" cy="90" r="86" className="gauge-face" />
        <circle cx="90" cy="90" r={r} className="gauge-track" />
        <circle
          cx="90"
          cy="90"
          r={r}
          className="gauge-fill"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - Math.max(0, Math.min(1, fraction)))}
          transform="rotate(-90 90 90)"
        />
      </svg>
      <span className="gauge-seconds">{seconds === null ? "∞" : seconds}</span>
      <span className="gauge-label">{label}</span>
    </div>
  );
}

function ResultBars({ right, wrong, none, total }: { right: number; wrong: number; none: number; total: number }) {
  const width = (n: number) => `${total > 0 ? (n / total) * 100 : 0}%`;
  return (
    <div className="result-bars">
      <div className="result-row right">
        <CheckIcon />
        <span className="bar" style={{ width: width(right) }} />
        <span className="n">{right}</span>
      </div>
      <div className="result-row wrong">
        <CrossIcon />
        <span className="bar" style={{ width: width(wrong) }} />
        <span className="n">{wrong}</span>
      </div>
      <div className="result-row none">
        <QuestionIcon />
        <span className="bar" style={{ width: width(none) }} />
        <span className="n">{none}</span>
      </div>
    </div>
  );
}

function Leaderboard({ t, players }: { t: Translate; players: PlayerView[] }) {
  const top = players.slice(0, 20);
  const columns = [top.slice(0, 10), top.slice(10, 20)].filter((c) => c.length > 0);
  return (
    <div className="leaderboard">
      {columns.map((column, c) => (
        <ol key={c}>
          {column.map((p) => (
            <li key={p.buzzer} className={p.rank !== null && p.rank <= 3 ? `top top-${p.rank}` : ""}>
              <span className="rank">{p.rank}</span>
              <span className="name">{playerName(t, p)}</span>
              <span className="score">{pad3(p.score)}</span>
            </li>
          ))}
        </ol>
      ))}
    </div>
  );
}

function FinalScores({ t, players, step }: { t: Translate; players: PlayerView[]; step: number }) {
  if (step >= 4) {
    return (
      <div className="center-slide">
        <h1 className="slide-title small">{t("final")}</h1>
        <Leaderboard t={t} players={players} />
      </div>
    );
  }
  // Steps 1..3 reveal third, second, then first place on a podium.
  const places = [2, 1, 3];
  return (
    <div className="center-slide">
      <h1 className="slide-title small">{step === 0 ? t("drumroll") : t("final")}</h1>
      <div className="podium">
        {places.map((place) => {
          const p = players[place - 1];
          const shown = step >= 4 - place;
          return (
            <div key={place} className={`podium-step place-${place}`}>
              <div className={`podium-name ${shown && p ? "shown" : ""}`}>
                {shown && p && (
                  <>
                    <span className="name">{playerName(t, p)}</span>
                    <span className="score">{pad3(p.score)}</span>
                  </>
                )}
              </div>
              <div className="podium-block">{place}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
