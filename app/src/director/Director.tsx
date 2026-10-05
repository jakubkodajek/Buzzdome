import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { api, useGameView, useRemainingMs } from "../api";
import { translator, useLang, type Translate } from "../i18n";
import { formatSeconds, phaseLabel } from "../phase";
import { useShortcuts } from "../shortcuts";
import { ANSWER_LETTERS, type GameView, type MonitorInfo, type SessionSummary } from "../types";
import { PlayersTable } from "./PlayersTable";
import { VirtualBuzzers } from "./VirtualBuzzers";

export function Director() {
  const [lang, setLang] = useLang();
  const t = translator(lang);
  const { view, receivedAt } = useGameView();
  const [error, setError] = useState<string | null>(null);
  const showError = useCallback((message: string) => setError(message), []);
  useShortcuts(view, showError);

  useEffect(() => {
    if (!error) return;
    const timer = setTimeout(() => setError(null), 4000);
    return () => clearTimeout(timer);
  }, [error]);

  return (
    <div className="director">
      <header className="director-header">
        <strong>{view?.quizTitle ?? t("appTitle")}</strong>
        {view && <span className="muted">{t("slide", { n: view.slideIndex + 1, total: view.slideCount })}</span>}
        <span className="spacer" />
        <ProjectorControls t={t} onError={showError} />
        <label className="inline">
          {t("language")}
          <select value={lang} onChange={(e) => setLang(e.target.value as "cs" | "en")}>
            <option value="cs">Čeština</option>
            <option value="en">English</option>
          </select>
        </label>
      </header>
      {error && <div className="toast" role="alert">{error}</div>}
      {view ? (
        <main className="director-main">
          <section className="panel slide-panel">
            <ShowControls t={t} view={view} receivedAt={receivedAt} onError={showError} />
            <CurrentSlide t={t} view={view} />
          </section>
          <section className="panel">
            <h2>{t("players")}</h2>
            <PlayersTable t={t} view={view} onError={showError} />
          </section>
          <section className="panel wide">
            <h2>{t("virtualBuzzers")}</h2>
            <VirtualBuzzers t={t} onError={showError} />
          </section>
        </main>
      ) : (
        <StartScreen t={t} onError={showError} />
      )}
    </div>
  );
}

function StartScreen({ t, onError }: { t: Translate; onError: (m: string) => void }) {
  const [last, setLast] = useState<SessionSummary | null>(null);
  useEffect(() => {
    api.lastSession().then(setLast).catch(() => setLast(null));
  }, []);

  const openQuiz = async () => {
    const path = await open({ multiple: false, filters: [{ name: "Quiz", extensions: ["json"] }] });
    if (typeof path === "string") api.startShowFromFile(path).catch((e) => onError(String(e)));
  };

  return (
    <main className="start">
      <p className="muted">{t("noShow")}</p>
      <div className="row">
        <button className="primary" onClick={openQuiz}>{t("openQuiz")}</button>
        <button onClick={() => api.startDemoShow().catch((e) => onError(String(e)))}>{t("demoQuiz")}</button>
      </div>
      {last && (
        <div className="row">
          <button onClick={() => api.resumeLastSession().catch((e) => onError(String(e)))}>{t("resume")}</button>
          <span className="muted">
            {t("resumeInfo", { title: last.meta.quizTitle, count: last.commandCount })} ·{" "}
            {new Date(last.meta.startedMs).toLocaleString()}
          </span>
        </div>
      )}
    </main>
  );
}

function ProjectorControls({ t, onError }: { t: Translate; onError: (m: string) => void }) {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState<number>(0);
  const [fullscreen, setFullscreen] = useState(true);

  useEffect(() => {
    api
      .listMonitors()
      .then((list) => {
        setMonitors(list);
        // Default to the second display, which is usually the projector.
        if (list.length > 1) setMonitor(list[1].index);
      })
      .catch((e) => onError(String(e)));
  }, [onError]);

  return (
    <div className="inline">
      {t("projector")}
      <select value={monitor} onChange={(e) => setMonitor(Number(e.target.value))}>
        {monitors.map((m) => (
          <option key={m.index} value={m.index}>
            {m.name} ({m.width}×{m.height})
          </option>
        ))}
      </select>
      <label className="inline">
        <input type="checkbox" checked={fullscreen} onChange={(e) => setFullscreen(e.target.checked)} />
        {t("fullscreen")}
      </label>
      <button onClick={() => api.openHostWindow(monitor, fullscreen).catch((e) => onError(String(e)))}>
        {t("openProjector")}
      </button>
      <button onClick={() => api.closeHostWindow().catch((e) => onError(String(e)))}>{t("closeProjector")}</button>
    </div>
  );
}

function ShowControls(props: { t: Translate; view: GameView; receivedAt: number; onError: (m: string) => void }) {
  const { t, view, receivedAt, onError } = props;
  const remaining = useRemainingMs(view, receivedAt);
  const run = (command: Parameters<typeof api.dispatch>[0]) => api.dispatch(command).catch((e) => onError(String(e)));
  const phase = view.phase;
  const buzzed = phase.phase === "question" && phase.step === "buzzed";
  const running = phase.phase === "question" && phase.step === "running";

  return (
    <div className="controls">
      <div className="status">
        <span className="badge">{phaseLabel(t, view)}</span>
        {view.clock && (
          <span className="clock">{remaining === null ? t("untimed") : formatSeconds(remaining)}</span>
        )}
        {phase.phase === "question" && view.slide?.type === "question" && view.slide.mode === "everyone" && (
          <span className="muted">{t("answered", { n: view.answeredCount, total: view.enabledCount })}</span>
        )}
      </div>
      <div className="row">
        <button className="primary big" onClick={() => run({ type: "next" })} disabled={buzzed}>
          {t("next")} ␣
        </button>
        {buzzed && (
          <>
            <button className="good big" onClick={() => run({ type: "judgeRight" })}>{t("right")} (Y)</button>
            <button className="bad big" onClick={() => run({ type: "judgeWrong" })}>{t("wrong")} (N)</button>
          </>
        )}
        {running && (
          <button onClick={() => run(view.clock?.running ? { type: "pauseClock" } : { type: "resumeClock" })}>
            {view.clock?.running ? t("pause") : t("resumeClock")} (P)
          </button>
        )}
        <button
          className={view.leaderboardVisible ? "active" : ""}
          onClick={() => run({ type: "showLeaderboard", visible: !view.leaderboardVisible })}
        >
          {t("leaderboard")} (S)
        </button>
        <select
          value=""
          onChange={(e) => e.target.value !== "" && run({ type: "goTo", slide: Number(e.target.value) })}
        >
          <option value="">{t("goTo")}</option>
          {Array.from({ length: view.slideCount }, (_, i) => (
            <option key={i} value={i}>
              {i + 1}
            </option>
          ))}
        </select>
      </div>
      {view.nextSlideTitle && <p className="muted">{t("nextUp", { title: view.nextSlideTitle })}</p>}
    </div>
  );
}

function CurrentSlide({ t, view }: { t: Translate; view: GameView }) {
  const slide = view.slide;
  if (!slide) return null;
  return (
    <div className="current-slide">
      {slide.type === "question" ? (
        <>
          <p className="muted">
            {slide.mode === "everyone" ? t("everyone") : t("fastestFinger")} · {t("points", { n: slide.points })}
          </p>
          <h3>{slide.text}</h3>
          <ol className="answers">
            {slide.answers.map((answer, i) => (
              <li key={i} className={i === slide.correct ? "correct" : ""}>
                <b>{ANSWER_LETTERS[i]}</b> {answer}
                {view.answerCounts && <span className="muted"> · {view.answerCounts[i]}</span>}
              </li>
            ))}
          </ol>
        </>
      ) : (
        <h3>{slide.title}</h3>
      )}
      {slide.notes && (
        <div className="notes">
          <b>{t("notes")}:</b> {slide.notes}
        </div>
      )}
    </div>
  );
}
