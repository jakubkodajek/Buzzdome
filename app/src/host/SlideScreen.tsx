import type { ReactNode } from "react";
import { timeLeft } from "../answer-time";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import { FINAL_LAST_STEP, MAX_EVERYONE_ANSWERS } from "../bindings/Constants";
import { scoreDigits, signed } from "../format";
import { playerName } from "../i18n";
import { CheckIcon, CrossIcon, PeopleIcon, QuestionIcon, TrophyIcon } from "../icons";
import type { QuestionPosition, SlideContext } from "../quiz";
import { SlideMedia, type ScreenMode } from "../slide-media";
import type { AnswerResults, ClockView, GameView, PlayerView, QuestionSlide } from "../types";
import "./slide.css";

export function ranked(view: GameView): PlayerView[] {
  return view.players.filter((p) => p.rank !== null);
}

/**
 * What the projector shows for a game state. Fed only by its props, so the
 * quiz editor and the Director draw the same screens (see `ScreenMode`).
 * Outside the show, empty texts show what belongs there and the right
 * answer is ticked. Parts the editor can select carry `data-element`.
 */
export function SlideScreen(props: { view: GameView; context: SlideContext; clock: ClockView | null; mode?: ScreenMode }) {
  const { view, context, clock, mode = "show" } = props;
  const { t } = useApp();
  const phase = view.phase;
  const placeholder = (text: string, empty: string) => (text || mode === "show" ? text : <span className="placeholder">{empty}</span>);
  switch (phase.phase) {
    case "lobby":
      return <Welcome view={view} />;
    case "billboard":
      return view.slide?.type === "billboard" ? (
        <div className="center-slide">
          <h1 className="slide-title" data-element="title">
            {placeholder(view.slide.title, t("titlePlaceholder"))}
          </h1>
          <SlideMedia slide={view.slide} className="slide-media" mode={mode} live={{ view, clock }} />
          {(view.slide.text || mode === "preview") && (
            <p className="slide-text" data-element="text">
              {placeholder(view.slide.text, t("textPlaceholder"))}
            </p>
          )}
        </div>
      ) : null;
    case "explanation":
      return view.slide?.type === "explanation" ? (
        <div className="explanation-screen">
          {context.explains ? (
            <ExplainedHeader explained={context.explains.question} n={context.explains.position.n} />
          ) : (
            mode !== "show" && <p className="explained-missing">{t("noQuestionToExplain")}</p>
          )}
          <SlideMedia slide={view.slide} className="explanation-media" mode={mode} live={{ view, clock }} />
          <p className="explanation-text" data-element="text">
            {placeholder(view.slide.text, t("explanationPlaceholder"))}
          </p>
        </div>
      ) : null;
    case "endOfRound":
      return (
        <div className="center-slide">
          <h1 className="slide-title small" data-element="title">
            {view.slide?.type === "endOfRound" ? placeholder(view.slide.title, t("endOfRound")) : t("endOfRound")}
          </h1>
          <Leaderboard players={ranked(view)} />
        </div>
      );
    case "final":
      return (
        <FinalScores
          players={ranked(view)}
          step={phase.step}
          // A final slide's own title, or the default one.
          title={(view.slide?.type === "final" && placeholder(view.slide.title, t("final"))) || t("final")}
          titled={view.slide?.type === "final"}
          sample={mode !== "show"}
        />
      );
    case "question":
      return view.slide?.type === "question" ? (
        <QuestionScreen
          view={view}
          slide={view.slide}
          clock={clock}
          position={context.position}
          mode={mode}
          placeholder={placeholder}
        />
      ) : null;
  }
}

/** Registration: every buzzer number, lit once it is pressed, with its team's name once paired. */
function Welcome({ view }: { view: GameView }) {
  const { t } = useApp();
  const names = new Map(view.players.map((p) => [p.buzzer, p.name]));
  const highest = Math.max(20, ...view.players.map((p) => p.buzzer));
  const cells = Array.from({ length: Math.min(highest, 100) }, (_, i) => i + 1);
  return (
    <div className="welcome">
      <header className="welcome-header">{view.quizTitle}</header>
      <div className="welcome-body">
        <p className="slide-text">{t("lobbyHint")}</p>
        <div className="register-grid">
          {cells.map((n) => (
            <span key={n} className={names.has(n) ? "on" : ""}>
              {n}
              {names.get(n) && <small>{names.get(n)}</small>}
            </span>
          ))}
        </div>
      </div>
      <footer className="welcome-footer">{t("registered", { n: view.players.length })}</footer>
    </div>
  );
}

/** The question an explanation slide explains, with its correct answer. */
function ExplainedHeader({ explained, n }: { explained: QuestionSlide; n: number }) {
  const { t } = useApp();
  const correct = explained.correct;
  return (
    <header className="explained">
      <span className="explained-number">{t("questionNumber", { n })}</span>
      <h2 className="explained-question">{explained.text}</h2>
      {correct !== null && correct < explained.answers.length && (
        <div className={`answer answer-${correct} is-correct explained-answer`}>
          <span className="letter">{ANSWER_LETTERS[correct]}</span>
          <span className="answer-text">{explained.answers[correct]}</span>
          <CheckIcon className="answer-check" />
        </div>
      )}
    </header>
  );
}

function QuestionScreen(props: {
  view: GameView;
  slide: QuestionSlide;
  clock: ClockView | null;
  position: QuestionPosition | null;
  mode: ScreenMode;
  placeholder: (text: string, empty: string) => ReactNode;
}) {
  const { view, slide: question, clock, position, mode, placeholder } = props;
  const { t } = useApp();
  const step = view.phase.phase === "question" ? view.phase.step : "ready";
  const closed = step === "closed" || step === "revealed";
  const remaining = clock?.remainingMs ?? null;
  const enabled = view.players.filter((p) => p.enabled);
  const waiting = enabled.filter((p) => !p.answered);

  let middle: ReactNode = null;
  if (view.results) {
    middle = <ResultBars results={view.results} />;
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
  const winnerId = view.phase.phase === "question" && view.phase.step === "revealed" ? view.phase.winner : null;
  const winner = view.players.find((p) => p.buzzer === winnerId);
  const barPlayer = buzzed ?? winner;
  const hasMedia = question.picture !== null || question.video !== null;

  return (
    <div className={hasMedia ? "question-screen has-media" : "question-screen"}>
      <div className="info-panel">
        <ClockGauge
          seconds={step === "ready" ? question.timeLimitS || null : closed ? 0 : remaining === null ? null : Math.ceil(remaining / 1000)}
          fraction={timeLeft(view.phase, clock)}
          label={position ? `${position.n}/${position.total}` : ""}
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
          <span className="digits">{clock?.points ?? question.points}</span>
        </div>
      </div>

      <h1 className="question-text" data-element="question">
        {placeholder(question.text, t("questionPlaceholder"))}
      </h1>
      {hasMedia && <SlideMedia slide={question} className="question-media" mode={mode} live={{ view, clock }} />}

      {question.answers.length > 0 && (
        <div className={question.answers.length > MAX_EVERYONE_ANSWERS ? "answers answers-6" : "answers"}>
          {question.answers.map((answer, i) => {
            const revealed = step === "revealed";
            const isCorrect = i === question.correct;
            return (
              <div
                key={i}
                className={["answer", `answer-${i}`, revealed ? (isCorrect ? "is-correct" : "is-dim") : ""].join(" ")}
                data-element={`answer-${i}`}
              >
                <span className="letter">{ANSWER_LETTERS[i]}</span>
                <span className="answer-text">{placeholder(answer, t("answerPlaceholder", { letter: ANSWER_LETTERS[i] }))}</span>
                {closed && view.answerCounts && <span className="answer-count">{view.answerCounts[i]}</span>}
                {(revealed || mode !== "show") && isCorrect && <CheckIcon className="answer-check" />}
              </div>
            );
          })}
        </div>
      )}

      {question.mode === "fastestFinger" && barPlayer && (
        <div className="buzz-bar">
          <span className="buzz-name">{playerName(t, barPlayer)}</span>
          {barPlayer.delta !== null && barPlayer.delta !== 0 && (
            <span className={`buzz-delta ${barPlayer.delta > 0 ? "plus" : "minus"}`}>{signed(barPlayer.delta)}</span>
          )}
        </div>
      )}
      {question.mode === "fastestFinger" && step === "running" && !barPlayer && <div className="buzz-bar idle">{t("waitingForBuzz")}</div>}
      {question.mode === "fastestFinger" && step === "closed" && remaining === 0 && <div className="buzz-bar idle">{t("timeUp")}</div>}
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

function ResultBars({ results }: { results: AnswerResults }) {
  const { right, wrong, none } = results;
  const total = right + wrong + none;
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

export function Leaderboard({ players }: { players: PlayerView[] }) {
  const { t } = useApp();
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
              <span className="score">{scoreDigits(p.score)}</span>
            </li>
          ))}
        </ol>
      ))}
    </div>
  );
}

interface FinalScoresProps {
  players: PlayerView[];
  step: number;
  title: ReactNode;
  /** The title belongs to a final slide, which the editor can select. */
  titled: boolean;
  /** Outside the show: empty places show where the names go. */
  sample: boolean;
}

function FinalScores({ players, step, title, titled, sample }: FinalScoresProps) {
  const { t } = useApp();
  const heading = (text: ReactNode) => (
    <h1 className="slide-title small" data-element={titled ? "title" : undefined}>
      {text}
    </h1>
  );
  if (step >= FINAL_LAST_STEP) {
    return (
      <div className="center-slide">
        {heading(title)}
        <Leaderboard players={players} />
      </div>
    );
  }
  // Steps 1..3 reveal third, second, then first place on a podium.
  const places = [2, 1, 3];
  return (
    <div className="center-slide">
      {heading(step === 0 ? t("drumroll") : title)}
      <div className="podium">
        {places.map((place) => {
          const p = players[place - 1];
          const shown = step >= FINAL_LAST_STEP - place;
          return (
            <div key={place} className={`podium-step place-${place}`}>
              <div className={`podium-name ${shown && (p || sample) ? "shown" : ""}`}>
                {shown && p && (
                  <>
                    <span className="name">{playerName(t, p)}</span>
                    <span className="score">{scoreDigits(p.score)}</span>
                  </>
                )}
                {shown && !p && sample && <span className="name placeholder">{t("place", { n: place })}</span>}
              </div>
              <div className="podium-block">{place}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
