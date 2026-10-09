import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api } from "../api";
import { AppProvider, useApp, useDispatch, useRun } from "../app-context";
import { clockSeconds } from "../format";
import { useBackendEvent, useClock, useGameView, useLastSession, useQuiz } from "../hooks";
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
import { useShortcuts } from "../keyboard";
import { phaseLabel } from "../phase";
import { questionPosition } from "../quiz";
import { playCue } from "../sound";
import type { GameView, Quiz } from "../types";
import { QuestionsTab } from "./QuestionsTab";
import { ReceiverTab } from "./ReceiverTab";
import { ResponsesTab } from "./ResponsesTab";
import { SettingsTab, useProjectorSettings, type ProjectorSettings } from "./SettingsTab";
import { TeamsTab } from "./TeamsTab";
import { VirtualBuzzers } from "./VirtualBuzzers";
import "./director.css";

type Tab = "questions" | "teams" | "responses" | "buzzers" | "receiver" | "settings";

/** The quizmaster's window. */
export function Director() {
  const [error, setError] = useState<string | null>(null);
  const notify = useCallback((message: string) => setError(message), []);

  useEffect(() => {
    if (!error) return;
    const timer = setTimeout(() => setError(null), 4000);
    return () => clearTimeout(timer);
  }, [error]);

  return (
    <AppProvider notify={notify}>
      <DirectorLayout />
      {error && (
        <div className="toast" role="alert">
          {error}
        </div>
      )}
    </AppProvider>
  );
}

function DirectorLayout() {
  const { t } = useApp();
  const view = useGameView();
  const quiz = useQuiz();
  const projector = useProjectorSettings();
  const [tab, setTab] = useState<Tab>("questions");
  useShortcuts(view, projector.toggle);
  // Sounds play here: this window is open for the whole show, the projector may not be.
  useBackendEvent("sound-cue", playCue);

  const tabs: [Tab, string][] = [
    ["questions", t("tabQuestions")],
    ["teams", t("tabTeams")],
    ["responses", t("tabResponses")],
    ["buzzers", t("tabBuzzers")],
    ["receiver", t("tabReceiver")],
    ["settings", t("tabSettings")],
  ];

  return (
    <div className="director">
      <CommandPanel view={view} projector={projector} />
      <div className="workspace">
        <nav className="tabs">
          {tabs.map(([id, label]) => (
            <button key={id} className={tab === id ? "tab active" : "tab"} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </nav>
        <div className="tab-body">
          <TabBody tab={tab} view={view} quiz={quiz} projector={projector} />
        </div>
        <StatusBar view={view} quiz={quiz} />
      </div>
    </div>
  );
}

function TabBody({ tab, view, quiz, projector }: { tab: Tab; view: GameView | null; quiz: Quiz | null; projector: ProjectorSettings }) {
  const { t } = useApp();
  // Setup tabs work without a show; the others need one.
  if (tab === "settings") return <SettingsTab projector={projector} />;
  if (tab === "receiver") return <ReceiverTab />;
  if (!view) return <p className="empty">{t("welcome")}</p>;
  switch (tab) {
    case "questions":
      return <QuestionsTab view={view} quiz={quiz} />;
    case "teams":
      return <TeamsTab view={view} />;
    case "responses":
      return <ResponsesTab view={view} />;
    case "buzzers":
      return <VirtualBuzzers />;
  }
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

/** Left command area: only the commands that make sense right now. */
function CommandPanel({ view, projector }: { view: GameView | null; projector: ProjectorSettings }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const last = useLastSession();

  const openQuiz = async () => {
    const path = await open({ multiple: false, filters: [{ name: "Quiz", extensions: ["json"] }] });
    if (typeof path === "string") run(api.startShowFromFile(path));
  };

  const phase = view?.phase;
  const buzzed = phase?.phase === "question" && phase.step === "buzzed";
  const running = phase?.phase === "question" && phase.step === "running";

  return (
    <aside className="command-panel">
      {!view ? (
        <>
          <CommandButton icon={<FolderIcon />} label={t("openQuiz")} onClick={openQuiz} />
          <CommandButton icon={<PlayIcon />} label={t("demoQuiz")} onClick={() => run(api.startDemoShow())} />
          {last && (
            <CommandButton
              icon={<NextIcon />}
              label={t("resume")}
              hint={t("resumeInfo", { title: last.meta.quizTitle, count: last.commandCount })}
              onClick={() => run(api.resumeLastSession())}
            />
          )}
        </>
      ) : (
        <>
          {!buzzed && <CommandButton icon={<NextIcon />} label={t("next")} hint="Space" onClick={() => dispatch({ type: "next" })} />}
          {buzzed && (
            <>
              <CommandButton icon={<CheckIcon />} label={t("right")} hint="Y" tone="good" onClick={() => dispatch({ type: "judgeRight" })} />
              <CommandButton icon={<CrossIcon />} label={t("wrong")} hint="N" tone="bad" onClick={() => dispatch({ type: "judgeWrong" })} />
            </>
          )}
          {running && (
            <>
              {view.clock?.running ? (
                <CommandButton icon={<PauseIcon />} label={t("pause")} hint="P" onClick={() => dispatch({ type: "pauseClock" })} />
              ) : (
                <CommandButton icon={<PlayIcon />} label={t("resumeClock")} hint="P" onClick={() => dispatch({ type: "resumeClock" })} />
              )}
              <CommandButton icon={<StopIcon />} label={t("endQuestion")} onClick={() => dispatch({ type: "closeQuestion" })} />
            </>
          )}
          <CommandButton
            icon={<ListIcon />}
            label={t("leaderboard")}
            hint="S"
            onClick={() => dispatch({ type: "showLeaderboard", visible: !view.leaderboardVisible })}
          />
        </>
      )}
      <CommandButton
        icon={<ScreenIcon />}
        label={`${t("projectorToggle")}: ${projector.isOpen ? t("closeProjector") : t("openProjector")}`}
        hint="M"
        onClick={projector.toggle}
      />
      <span className="spacer" />
      {view && <CommandButton icon={<PowerIcon />} label={t("endShow")} tone="bad" onClick={() => run(api.endShow())} />}
    </aside>
  );
}

function StatusBar({ view, quiz }: { view: GameView | null; quiz: Quiz | null }) {
  const { t } = useApp();
  const clock = useClock(view);
  if (!view) return <footer className="status-bar">{t("noShow")}</footer>;
  const position = questionPosition(quiz, view.slideIndex);
  return (
    <footer className="status-bar">
      <span>{view.quizTitle}</span>
      <span>{t("slide", { n: view.slideIndex + 1, total: view.slideCount })}</span>
      {position && (
        <span>
          {t("colQuestionNo")} {position.n}/{position.total}
        </span>
      )}
      <span className="status-phase">{phaseLabel(t, view)}</span>
      {clock && <span className="status-clock">{clock.remainingMs === null ? t("untimed") : clockSeconds(clock.remainingMs)}</span>}
      <span className="spacer" />
      <span>{t("registered", { n: view.players.length })}</span>
    </footer>
  );
}
