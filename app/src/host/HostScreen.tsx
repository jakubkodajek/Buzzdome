import { useCallback } from "react";
import { useGameView, useRemainingMs, useSoundCues } from "../api";
import { playerName, translator, useLang, type Translate } from "../i18n";
import { formatSeconds } from "../phase";
import { useShortcuts } from "../shortcuts";
import { playCue } from "../sound";
import { ANSWER_LETTERS, type GameView, type PlayerView, type Question } from "../types";

/** The projector. Renders the game state; owns nothing. */
export function HostScreen() {
  const [lang] = useLang();
  const t = translator(lang);
  const { view, receivedAt } = useGameView();
  const ignore = useCallback(() => {}, []);
  useShortcuts(view, ignore);
  useSoundCues(playCue);

  if (!view) return <div className="host" />;
  return (
    <div className="host">
      <HostContent t={t} view={view} receivedAt={receivedAt} />
      {view.leaderboardVisible && (
        <div className="host-overlay">
          <Leaderboard t={t} players={ranked(view)} />
        </div>
      )}
    </div>
  );
}

function ranked(view: GameView): PlayerView[] {
  return view.players.filter((p) => p.rank !== null);
}

function HostContent({ t, view, receivedAt }: { t: Translate; view: GameView; receivedAt: number }) {
  const phase = view.phase;
  switch (phase.phase) {
    case "lobby":
      return (
        <div className="host-center">
          <h1>{view.quizTitle}</h1>
          <p className="host-sub">{t("lobbyHint")}</p>
          <div className="lobby-grid">
            {view.players.map((p) => (
              <span key={p.buzzer} className="lobby-chip">{playerName(t, p)}</span>
            ))}
          </div>
          <p className="host-sub">{t("registered", { n: view.players.length })}</p>
        </div>
      );
    case "billboard":
      return view.slide?.type === "billboard" ? (
        <div className="host-center">
          <h1>{view.slide.title}</h1>
          {view.slide.text && <p className="host-sub">{view.slide.text}</p>}
        </div>
      ) : null;
    case "endOfRound":
      return (
        <div className="host-center">
          <h1>{view.slide?.type === "endOfRound" ? view.slide.title : t("endOfRound")}</h1>
          <Leaderboard t={t} players={ranked(view)} />
        </div>
      );
    case "final":
      return <FinalScores t={t} players={ranked(view)} step={phase.step} />;
    case "question":
      return view.slide?.type === "question" ? (
        <QuestionScreen t={t} view={view} question={view.slide} receivedAt={receivedAt} />
      ) : null;
  }
}

function QuestionScreen(props: { t: Translate; view: GameView; question: Question; receivedAt: number }) {
  const { t, view, question, receivedAt } = props;
  const remaining = useRemainingMs(view, receivedAt);
  const phase = view.phase;
  const step = phase.phase === "question" ? phase.step : "ready";
  const revealed = step === "revealed";
  const buzzedPlayer = view.players.find((p) => p.buzzer === view.buzzed);
  const counts = view.answerCounts;
  const totalAnswers = counts?.reduce((a, b) => a + b, 0) ?? 0;

  return (
    <div className="host-question">
      <div className="info-bar">
        <span className="clock-badge">
          {step === "ready" ? (question.timeLimitS || "∞") : remaining === null ? "∞" : formatSeconds(remaining)}
        </span>
        <span>
          {question.mode === "everyone"
            ? t("answered", { n: view.answeredCount, total: view.enabledCount })
            : step === "running"
              ? t("waitingForBuzz")
              : ""}
        </span>
        <span className="points-badge">🏆 {question.points}</span>
      </div>
      <h1 className="question-text">{question.text}</h1>
      {question.answers.length > 0 && (
        <div className="answer-grid">
          {question.answers.map((answer, i) => {
            const isCorrect = i === question.correct;
            const share = counts && totalAnswers > 0 ? counts[i] / totalAnswers : 0;
            return (
              <div key={i} className={["answer", `answer-${i}`, revealed ? (isCorrect ? "correct" : "dim") : ""].join(" ")}>
                <span className="letter">{ANSWER_LETTERS[i]}</span>
                <span className="answer-text">{answer}</span>
                {counts && <span className="count">{counts[i]}</span>}
                {counts && <span className="bar" style={{ width: `${share * 100}%` }} />}
              </div>
            );
          })}
        </div>
      )}
      {step === "buzzed" && buzzedPlayer && (
        <div className="buzz-banner">{playerName(t, buzzedPlayer)}</div>
      )}
      {step === "closed" && remaining === 0 && <div className="buzz-banner subtle">{t("timeUp")}</div>}
      {revealed && question.mode === "fastestFinger" && <RoundDeltas t={t} players={view.players} />}
    </div>
  );
}

function RoundDeltas({ t, players }: { t: Translate; players: PlayerView[] }) {
  const changed = players.filter((p) => p.delta);
  if (changed.length === 0) return null;
  return (
    <div className="deltas">
      {changed.map((p) => (
        <span key={p.buzzer} className={(p.delta ?? 0) > 0 ? "good-text" : "bad-text"}>
          {playerName(t, p)} {(p.delta ?? 0) > 0 ? `+${p.delta}` : p.delta}
        </span>
      ))}
    </div>
  );
}

function Leaderboard({ t, players }: { t: Translate; players: PlayerView[] }) {
  return (
    <ol className="leaderboard">
      {players.slice(0, 20).map((p) => (
        <li key={p.buzzer} className={p.rank !== null && p.rank <= 3 ? `medal-${p.rank}` : ""}>
          <span className="rank">{p.rank}</span>
          <span className="name">{playerName(t, p)}</span>
          <span className="score">{p.score}</span>
        </li>
      ))}
    </ol>
  );
}

function FinalScores({ t, players, step }: { t: Translate; players: PlayerView[]; step: number }) {
  if (step >= 4) {
    return (
      <div className="host-center">
        <h1>{t("final")}</h1>
        <Leaderboard t={t} players={players} />
      </div>
    );
  }
  // Steps 1..3 reveal third, second, then first place.
  const shown = [3, 2, 1].slice(0, step).map((place) => players[place - 1]);
  return (
    <div className="host-center">
      <h1>{step === 0 ? t("drumroll") : t("final")}</h1>
      <div className="podium">
        {shown.map(
          (p, i) =>
            p && (
              <div key={p.buzzer} className={`podium-place medal-${3 - i}`}>
                <span className="place">{t("place", { n: 3 - i })}</span>
                <span className="name">{playerName(t, p)}</span>
                <span className="score">{p.score}</span>
              </div>
            ),
        )}
      </div>
    </div>
  );
}
