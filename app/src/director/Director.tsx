import { ask, open } from "@tauri-apps/plugin-dialog";
import { useCallback, useRef, useState, type ReactNode } from "react";
import { api } from "../api";
import { AppProvider, useApp, useDispatch, useRun } from "../app-context";
import { playerLevel } from "../audio/clip-player";
import { playCue } from "../audio/cues";
import { useSlideSound, type SlideSound } from "../audio/slide-sound";
import { clockSeconds } from "../format";
import { EditorScreen } from "../editor/EditorScreen";
import { errorText } from "../messages";
import { useBackendEvent, useClock, useGameView, useLastSession, useLearnProgress, useMediaRoot, usePlayerVolume, useQuiz, useShowNotSaved } from "../hooks";
import {
  BackIcon,
  CheckIcon,
  CrossIcon,
  EditIcon,
  FolderIcon,
  ListIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PowerIcon,
  ScreenIcon,
  SoundIcon,
  StopIcon,
  VideoIcon,
} from "../icons";
import { keyHint, KEYS, useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import { phaseLabel } from "../phase";
import { quizOpenFilter, slideContext } from "../quiz";
import { ThemeContext, ThemesProvider, useResolvedTheme, useThemes } from "../theme/context";
import { isThemeMissing } from "../theme/themes";
import type { GameView, Quiz } from "../types";
import { QuestionsTab } from "./QuestionsTab";
import { ReceiverTab } from "./ReceiverTab";
import { MediaPlayerTab } from "./MediaPlayerTab";
import { ResponsesTab } from "./ResponsesTab";
import { SettingsTab, useProjectorSettings, type ProjectorSettings } from "./SettingsTab";
import { TeamsTab } from "./TeamsTab";
import { useMediaPlayer, type MediaPlayer } from "./useMediaPlayer";
import { VirtualBuzzers } from "./VirtualBuzzers";
import "./director.css";

type Tab = "questions" | "teams" | "responses" | "buzzers" | "media" | "receiver" | "settings";

/** The show controls, the quiz editor, or the show controls for a test run of the edited quiz. */
type Screen = "show" | "editor" | "testRun";

/** How long a message stays on screen. */
const TOAST_MS = 4000;
/** The most messages on screen at once; an older one makes way. */
const MAX_TOASTS = 3;

/** The quizmaster's window. */
export function Director() {
  const [toasts, setToasts] = useState<{ id: number; message: string }[]>([]);
  const lastId = useRef(0);
  // Each message stays its own time, so a second one does not hide the first.
  const notify = useCallback((message: string) => {
    const id = ++lastId.current;
    setToasts((shown) => [...shown.filter((toast) => toast.message !== message), { id, message }].slice(-MAX_TOASTS));
    setTimeout(() => setToasts((shown) => shown.filter((toast) => toast.id !== id)), TOAST_MS);
  }, []);

  return (
    <AppProvider notify={notify}>
      <ThemesProvider>
        <DirectorScreens />
      </ThemesProvider>
      <div className="toasts">
        {toasts.map((toast) => (
          <div key={toast.id} className="toast" role="alert">
            {toast.message}
          </div>
        ))}
      </div>
    </AppProvider>
  );
}

function DirectorScreens() {
  const { t, notify } = useApp();
  const [screen, setScreen] = useState<Screen>("show");
  useBackendEvent("host-problem", (message) => notify(t("hostProblem", { message })));
  // Provided above the show controls: their hooks (the slide sound) resolve media too.
  const mediaRoot = useMediaRoot();
  if (screen === "editor") return <EditorScreen onTestRun={() => setScreen("testRun")} onExit={() => setScreen("show")} />;
  return (
    <MediaRootContext value={mediaRoot}>
      <DirectorLayout testRun={screen === "testRun"} onEditor={() => setScreen("editor")} />
    </MediaRootContext>
  );
}

/**
 * The show controls. In a test run the show ends back in the editor;
 * `onEditor` opens it.
 */
function DirectorLayout({ testRun, onEditor }: { testRun: boolean; onEditor: () => void }) {
  const { t } = useApp();
  const view = useGameView();
  const quiz = useQuiz();
  const theme = useResolvedTheme(quiz?.theme ?? null);
  const projector = useProjectorSettings();
  const [tab, setTab] = useState<Tab>("questions");
  useShortcuts(view, projector.toggle);
  // Sounds play here: this window is open for the whole show, the projector may not be.
  const level = playerLevel(usePlayerVolume());
  useBackendEvent("sound-cue", (cue) => playCue(cue, level));
  const sound = useSlideSound(view);
  // Here too, so what plays goes on and ends whichever tab is open.
  const player = useMediaPlayer();

  const tabs: [Tab, string][] = [
    ["questions", t("tabQuestions")],
    ["teams", t("tabTeams")],
    ["responses", t("tabResponses")],
    ["buzzers", t("tabBuzzers")],
    ["media", t("tabMedia")],
    ["receiver", t("tabReceiver")],
    ["settings", t("tabSettings")],
  ];

  return (
    <div className="director">
      <CommandPanel view={view} projector={projector} sound={sound} testRun={testRun} onEditor={onEditor} />
      <div className="workspace">
        <nav className="tabs" role="tablist">
          {tabs.map(([id, label]) => (
            <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "tab active" : "tab"} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </nav>
        <NotSavedBanner />
        <LearnBanner />
        <div className="tab-body" role="tabpanel">
          <ThemeContext value={theme}>
            <TabBody tab={tab} view={view} quiz={quiz} projector={projector} player={player} sound={sound} />
          </ThemeContext>
        </div>
        <StatusBar view={view} quiz={quiz} testRun={testRun} />
      </div>
    </div>
  );
}

/** Shown while the show's journal refuses writes, when nothing in the show changes. */
function NotSavedBanner() {
  const { t } = useApp();
  const problem = useShowNotSaved();
  if (!problem) return null;
  return (
    <div className="not-saved-banner" role="alert">
      {t("notSavedBanner", { problem: errorText(t, problem) })}
    </div>
  );
}

/** Shown while a buzzer button is being learned, when presses do not play. */
function LearnBanner() {
  const { t } = useApp();
  const run = useRun();
  const learn = useLearnProgress();
  if (!learn?.active) return null;
  return (
    <div className="learn-banner" role="status">
      <span>{t("learnBanner")}</span>
      <button onClick={() => run(api.receiverLearnCancel())}>{t("learnCancel")}</button>
    </div>
  );
}

interface TabBodyProps {
  tab: Tab;
  view: GameView | null;
  quiz: Quiz | null;
  projector: ProjectorSettings;
  player: MediaPlayer;
  sound: SlideSound;
}

function TabBody({ tab, view, quiz, projector, player, sound }: TabBodyProps) {
  const { t } = useApp();
  // Setup tabs work without a show; the others need one.
  if (tab === "settings") return <SettingsTab projector={projector} />;
  if (tab === "receiver") return <ReceiverTab />;
  // Team names can be prepared before a show, and the media player plays without one.
  if (tab === "teams") return <TeamsTab view={view} />;
  if (tab === "media") return <MediaPlayerTab player={player} sound={sound} view={view} />;
  if (!view) return <p className="empty">{t("welcome")}</p>;
  switch (tab) {
    case "questions":
      return <QuestionsTab view={view} quiz={quiz} />;
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
function CommandPanel(props: { view: GameView | null; projector: ProjectorSettings; sound: SlideSound; testRun: boolean; onEditor: () => void }) {
  const { view, projector, sound, testRun, onEditor } = props;
  const { t, notify } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const last = useLastSession();
  // Why Resume failed, kept on screen until another show loads: the show
  // waits on it (A1).
  const [resumeProblem, setResumeProblem] = useState<unknown>(null);
  useBackendEvent("show-changed", () => setResumeProblem(null));
  const resume = () => {
    setResumeProblem(null);
    api.resumeLastSession().then((warnings) => warnings.length > 0 && notify(warnings.map((w) => errorText(t, w)).join(" ")), setResumeProblem);
  };
  const problem = resumeProblem ?? last.problem;

  const openQuiz = async () => {
    const path = await open({ multiple: false, filters: [quizOpenFilter(t)] });
    if (typeof path === "string") run(api.startShowFromFile(path));
  };

  const controls = view?.controls;
  const hint = (key: string) => keyHint(t, key);

  return (
    <aside className="command-panel">
      {!view ? (
        <>
          <CommandButton icon={<FolderIcon />} label={t("openQuiz")} onClick={openQuiz} />
          <CommandButton icon={<PlayIcon />} label={t("demoQuiz")} onClick={() => run(api.startDemoShow())} />
          <CommandButton icon={<EditIcon />} label={t("quizEditor")} onClick={onEditor} />
          {last.session && (
            <CommandButton
              icon={<NextIcon />}
              label={t("resume")}
              hint={t("resumeInfo", { title: last.session.meta.quizTitle, count: last.session.commandCount })}
              onClick={resume}
            />
          )}
          {problem !== null && (
            <p className="command-problem" role="alert">
              {t("resumeProblem", { problem: errorText(t, problem) })}
            </p>
          )}
        </>
      ) : (
        <>
          {controls?.next && <CommandButton icon={<NextIcon />} label={t("next")} hint={hint(KEYS.next)} onClick={() => dispatch({ type: "next" })} />}
          {controls?.judge && (
            <>
              <CommandButton icon={<CheckIcon />} label={t("right")} hint={hint(KEYS.right)} tone="good" onClick={() => dispatch({ type: "judgeRight" })} />
              <CommandButton icon={<CrossIcon />} label={t("wrong")} hint={hint(KEYS.wrong)} tone="bad" onClick={() => dispatch({ type: "judgeWrong" })} />
            </>
          )}
          {controls?.pauseClock && (
            <CommandButton icon={<PauseIcon />} label={t("pause")} hint={hint(KEYS.clock)} onClick={() => dispatch({ type: "pauseClock" })} />
          )}
          {controls?.resumeClock && (
            <CommandButton icon={<PlayIcon />} label={t("resumeClock")} hint={hint(KEYS.clock)} onClick={() => dispatch({ type: "resumeClock" })} />
          )}
          {controls?.closeQuestion && (
            <CommandButton icon={<StopIcon />} label={t("endQuestion")} onClick={() => dispatch({ type: "closeQuestion" })} />
          )}
          {sound.available &&
            (sound.playing ? (
              <CommandButton icon={<StopIcon />} label={t("stopSound")} onClick={sound.stop} />
            ) : (
              <CommandButton icon={<SoundIcon />} label={t("playSound")} onClick={sound.play} />
            ))}
          {controls?.playVideo && <CommandButton icon={<VideoIcon />} label={t("playVideo")} onClick={() => dispatch({ type: "playVideo" })} />}
          <CommandButton
            icon={<ListIcon />}
            label={t("leaderboard")}
            hint={hint(KEYS.leaderboard)}
            onClick={() => dispatch({ type: "showLeaderboard", visible: !view.leaderboardVisible })}
          />
        </>
      )}
      <CommandButton
        icon={<ScreenIcon />}
        label={`${t("projectorToggle")}: ${projector.isOpen ? t("closeProjector") : t("openProjector")}`}
        hint={hint(KEYS.projector)}
        onClick={projector.toggle}
      />
      <span className="spacer" />
      {view &&
        (testRun ? (
          <CommandButton icon={<BackIcon />} label={t("backToEditor")} onClick={() => run(api.endShow().then(onEditor))} />
        ) : (
          <CommandButton
            icon={<PowerIcon />}
            label={t("endShow")}
            tone="bad"
            onClick={() => run(ask(t("endShowConfirm"), { kind: "warning" }).then((yes) => (yes ? api.endShow() : undefined)))}
          />
        ))}
    </aside>
  );
}

function StatusBar({ view, quiz, testRun }: { view: GameView | null; quiz: Quiz | null; testRun: boolean }) {
  const { t } = useApp();
  const clock = useClock(view);
  const themes = useThemes();
  if (!view) return <footer className="status-bar">{t("noShow")}</footer>;
  const position = slideContext(quiz, view.slideIndex).position;
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
      {quiz?.theme && isThemeMissing(themes, quiz.theme) && <span className="status-warning">{t("themeNotInstalled", { id: quiz.theme })}</span>}
      {testRun && <span className="status-test-run">{t("testRunActive")}</span>}
      <span>{t("registered", { n: view.players.length })}</span>
    </footer>
  );
}
