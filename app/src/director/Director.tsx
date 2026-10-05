import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, questionNumber, useGameView, useQuiz, useRemainingMs } from "../api";
import { translator, useLang, type Translate } from "../i18n";
import {
  CheckIcon,
  CrossIcon,
  FolderIcon,
  ListIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PowerIcon,
  ScreenIcon,
  StopIcon,
} from "../icons";
import { formatSeconds, phaseLabel } from "../phase";
import { useShortcuts } from "../shortcuts";
import type { Command, GameView, SessionSummary } from "../types";
import { QuestionsTab } from "./QuestionsTab";
import { ResponsesTab } from "./ResponsesTab";
import { SettingsTab, useProjectorSettings } from "./SettingsTab";
import { TeamsTab } from "./TeamsTab";
import { VirtualBuzzers } from "./VirtualBuzzers";
import "./director.css";

type Tab = "questions" | "teams" | "responses" | "buzzers" | "settings";

export function Director() {
  const [lang, setLang] = useLang();
  const t = translator(lang);
  const { view, receivedAt } = useGameView();
  const quiz = useQuiz(view);
  const [tab, setTab] = useState<Tab>("questions");
  const [error, setError] = useState<string | null>(null);
  const showError = useCallback((message: string) => setError(message), []);
  const projector = useProjectorSettings(showError);
  useShortcuts(view, showError);

  useEffect(() => {
    if (!error) return;
    const timer = setTimeout(() => setError(null), 4000);
    return () => clearTimeout(timer);
  }, [error]);

  // M toggles the projector, as in the original.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
      if (e.key.toLowerCase() === "m" && !e.metaKey && !e.ctrlKey) projector.toggle();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [projector]);

  const tabs: [Tab, string][] = [
    ["questions", t("tabQuestions")],
    ["teams", t("tabTeams")],
    ["responses", t("tabResponses")],
    ["buzzers", t("tabBuzzers")],
    ["settings", t("tabSettings")],
  ];

  return (
    <div className="director">
      <CommandPanel t={t} view={view} projectorOpen={projector.isOpen} onProjector={projector.toggle} onError={showError} />
      <div className="workspace">
        <nav className="tabs">
          {tabs.map(([id, label]) => (
            <button key={id} className={tab === id ? "tab active" : "tab"} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </nav>
        <div className="tab-body">
          {tab === "settings" ? (
            <SettingsTab t={t} lang={lang} setLang={setLang} projector={projector} />
          ) : !view ? (
            <p className="empty">{t("welcome")}</p>
          ) : tab === "questions" ? (
            <QuestionsTab t={t} view={view} quiz={quiz} onError={showError} />
          ) : tab === "teams" ? (
            <TeamsTab t={t} view={view} onError={showError} />
          ) : tab === "responses" ? (
            <ResponsesTab t={t} view={view} />
          ) : (
            <VirtualBuzzers t={t} onError={showError} />
          )}
        </div>
        <StatusBar t={t} view={view} receivedAt={receivedAt} questionNo={view ? questionNumber(quiz, view.slideIndex) : null} />
      </div>
      {error && (
        <div className="toast" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}

function CommandButton(props: { icon: ReactNode; label: string; hint?: string; tone?: "good" | "bad"; onClick: () => void }) {
  return (
    <button className={`command ${props.tone ?? ""}`} onClick={props.onClick}>
      <span className="command-icon">{props.icon}</span>
      <span className="command-label">
        {props.label}
        {props.hint && <small>{props.hint}</small>}
      </span>
    </button>
  );
}

interface CommandPanelProps {
  t: Translate;
  view: GameView | null;
  projectorOpen: boolean;
  onProjector: () => void;
  onError: (message: string) => void;
}

/** Left command area: only the commands that make sense right now. */
function CommandPanel({ t, view, projectorOpen, onProjector, onError }: CommandPanelProps) {
  const run = (command: Command) => api.dispatch(command).catch((e) => onError(String(e)));
  const [last, setLast] = useState<SessionSummary | null>(null);
  useEffect(() => {
    if (!view) api.lastSession().then(setLast).catch(() => setLast(null));
  }, [view]);

  const openQuiz = async () => {
    const path = await open({ multiple: false, filters: [{ name: "Quiz", extensions: ["json"] }] });
    if (typeof path === "string") api.startShowFromFile(path).catch((e) => onError(String(e)));
  };

  const phase = view?.phase;
  const buzzed = phase?.phase === "question" && phase.step === "buzzed";
  const running = phase?.phase === "question" && phase.step === "running";

  return (
    <aside className="command-panel">
      {!view ? (
        <>
          <CommandButton icon={<FolderIcon />} label={t("openQuiz")} onClick={openQuiz} />
          <CommandButton icon={<PlayIcon />} label={t("demoQuiz")} onClick={() => api.startDemoShow().catch((e) => onError(String(e)))} />
          {last && (
            <CommandButton
              icon={<NextIcon />}
              label={t("resume")}
              hint={t("resumeInfo", { title: last.meta.quizTitle, count: last.commandCount })}
              onClick={() => api.resumeLastSession().catch((e) => onError(String(e)))}
            />
          )}
        </>
      ) : (
        <>
          {!buzzed && <CommandButton icon={<NextIcon />} label={t("next")} hint="Space" onClick={() => run({ type: "next" })} />}
          {buzzed && (
            <>
              <CommandButton icon={<CheckIcon />} label={t("right")} hint="Y" tone="good" onClick={() => run({ type: "judgeRight" })} />
              <CommandButton icon={<CrossIcon />} label={t("wrong")} hint="N" tone="bad" onClick={() => run({ type: "judgeWrong" })} />
            </>
          )}
          {running && (
            <>
              {view.clock?.running ? (
                <CommandButton icon={<PauseIcon />} label={t("pause")} hint="P" onClick={() => run({ type: "pauseClock" })} />
              ) : (
                <CommandButton icon={<PlayIcon />} label={t("resumeClock")} hint="P" onClick={() => run({ type: "resumeClock" })} />
              )}
              <CommandButton icon={<StopIcon />} label={t("endQuestion")} onClick={() => run({ type: "closeQuestion" })} />
            </>
          )}
          <CommandButton
            icon={<ListIcon />}
            label={t("leaderboard")}
            hint="S"
            onClick={() => run({ type: "showLeaderboard", visible: !view.leaderboardVisible })}
          />
        </>
      )}
      <CommandButton
        icon={<ScreenIcon />}
        label={`${t("projectorToggle")}: ${projectorOpen ? t("closeProjector") : t("openProjector")}`}
        hint="M"
        onClick={onProjector}
      />
      <span className="spacer" />
      {view && <CommandButton icon={<PowerIcon />} label={t("endShow")} tone="bad" onClick={() => api.endShow().catch((e) => onError(String(e)))} />}
    </aside>
  );
}

function StatusBar(props: { t: Translate; view: GameView | null; receivedAt: number; questionNo: { n: number; total: number } | null }) {
  const { t, view, receivedAt, questionNo } = props;
  const remaining = useRemainingMs(view, receivedAt);
  if (!view) return <footer className="status-bar">{t("noShow")}</footer>;
  return (
    <footer className="status-bar">
      <span>{view.quizTitle}</span>
      <span>{t("slide", { n: view.slideIndex + 1, total: view.slideCount })}</span>
      {questionNo && (
        <span>
          {t("colQuestionNo")} {questionNo.n}/{questionNo.total}
        </span>
      )}
      <span className="status-phase">{phaseLabel(t, view)}</span>
      {view.clock && <span className="status-clock">{remaining === null ? t("untimed") : formatSeconds(remaining)}</span>}
      <span className="spacer" />
      <span>{t("registered", { n: view.players.length })}</span>
    </footer>
  );
}
