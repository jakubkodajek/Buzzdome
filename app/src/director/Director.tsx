import { useCallback, useRef, useState, type ReactNode } from "react";
import { api } from "../api";
import { AppProvider, useApp, useDispatch, useRun, useStepShown } from "../app-context";
import { otherSoundPlays, useShowSounds } from "../audio/show-sounds";
import { useSlideSound, type SlideSound } from "../audio/slide-sound";
import { clockSeconds } from "../format";
import { EditorScreen } from "../editor/EditorScreen";
import { ErrorBoundary, RenderFailed } from "../error-boundary";
import { errorText, isAppError } from "../messages";
import {
  useBackendEvent,
  useClock,
  useGameView,
  useLastSession,
  useLatest,
  useLearnProgress,
  useMediaRoot,
  useQuiz,
  useRecentFiles,
  useShowNotSaved,
} from "../hooks";
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
  RewindIcon,
  ScreenIcon,
  SoundIcon,
  StopIcon,
  VideoIcon,
} from "../icons";
import { keyHint, KEYS, useRemote, useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import { phaseLabel } from "../phase";
import { quizPicker, slideContext } from "../quiz";
import { RecentFileList } from "../recent-files";
import { ThemeContext, ThemesProvider, useResolvedTheme, useThemes } from "../theme/context";
import { isThemeMissing } from "../theme/themes";
import type { AutoStepView, GameView, Quiz } from "../types";
import { QuestionsTab } from "./QuestionsTab";
import { LightsTab } from "./LightsTab";
import { ReceiverTab } from "./ReceiverTab";
import { MediaPlayerTab } from "./MediaPlayerTab";
import { ResponsesTab } from "./ResponsesTab";
import { ResultsTab } from "./ResultsTab";
import { SettingsTab, useProjectorSettings, type ProjectorSettings } from "./SettingsTab";
import { TeamsTab } from "./TeamsTab";
import { confirmAction } from "../confirm";
import { useMediaPlayer, type MediaPlayer } from "./useMediaPlayer";
import { restartOutcome } from "./restart";
import { VirtualBuzzers } from "./VirtualBuzzers";
import "./director.css";

type Tab = "questions" | "teams" | "responses" | "results" | "buzzers" | "media" | "receiver" | "lights" | "settings";

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
  if (screen === "editor") {
    return (
      // An editor that fails to render keeps the way back to the show.
      <ErrorBoundary
        fallback={(message, retry) => (
          <RenderFailed t={t} text={t("partFailed", { message })} retry={retry}>
            <button onClick={() => setScreen("show")}>{t("backToDirector")}</button>
          </RenderFailed>
        )}
      >
        <EditorScreen onTestRun={() => setScreen("testRun")} onExit={() => setScreen("show")} />
      </ErrorBoundary>
    );
  }
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
  useStepShown(view);
  useShortcuts(view, projector.toggle);
  useRemote(view, projector.toggle);
  // Sounds play here: this window is open for the whole show, the projector may not be.
  const sound = useSlideSound(view);
  // Here too, so what plays goes on and ends whichever tab is open.
  const player = useMediaPlayer();
  useShowSounds(view, quiz, otherSoundPlays(view, sound.playing, player.now));
  const showStart = useShowStart();

  const tabs: [Tab, string][] = [
    ["questions", t("tabQuestions")],
    ["teams", t("tabTeams")],
    ["responses", t("tabResponses")],
    ["results", t("tabResults")],
    ["buzzers", t("tabBuzzers")],
    ["media", t("tabMedia")],
    ["receiver", t("tabReceiver")],
    ["lights", t("tabLights")],
    ["settings", t("tabSettings")],
  ];

  return (
    // `data-step`: the step on screen, which the E2E tests wait for before a step press (StepGuard.shown).
    <div className="director" data-step={view?.stepNo}>
      <CommandPanel view={view} projector={projector} sound={sound} showStart={showStart} testRun={testRun} onEditor={onEditor} />
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
            {/* A tab that fails to render keeps the rest of the window, and its keys, working. */}
            <ErrorBoundary key={tab} fallback={(message, retry) => <RenderFailed t={t} text={t("partFailed", { message })} retry={retry} />}>
              <TabBody tab={tab} view={view} quiz={quiz} projector={projector} player={player} sound={sound} showStart={showStart} />
            </ErrorBoundary>
          </ThemeContext>
        </div>
        <StatusBar view={view} quiz={quiz} testRun={testRun} />
      </div>
    </div>
  );
}

/** Shown while the show's journal refuses writes, when nothing in the show changes, or a write does not finish. */
function NotSavedBanner() {
  const { t } = useApp();
  const problem = useShowNotSaved();
  if (!problem) return null;
  return (
    <div className="not-saved-banner" role="alert">
      {/* A write that hangs keeps the presses for when it finishes, as its own text says. */}
      {problem.code === "saveHangs" ? errorText(t, problem) : t("notSavedBanner", { problem: errorText(t, problem) })}
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
  showStart: ShowStart;
}

function TabBody({ tab, view, quiz, projector, player, sound, showStart }: TabBodyProps) {
  // Setup tabs work without a show; the others need one.
  if (tab === "settings") return <SettingsTab projector={projector} />;
  if (tab === "receiver") return <ReceiverTab />;
  if (tab === "lights") return <LightsTab />;
  // Team names can be prepared before a show, and the media player plays without one.
  if (tab === "teams") return <TeamsTab view={view} />;
  if (tab === "media") return <MediaPlayerTab player={player} sound={sound} view={view} />;
  if (!view) return <StartPane showStart={showStart} />;
  switch (tab) {
    case "questions":
      return <QuestionsTab view={view} quiz={quiz} />;
    case "responses":
      return <ResponsesTab view={view} />;
    case "results":
      return <ResultsTab view={view} quiz={quiz} />;
    case "buzzers":
      return <VirtualBuzzers />;
  }
}

/** The show's tabs before a show: the recent quiz files, each starting a show in a click (spec SF-1). */
function StartPane({ showStart }: { showStart: ShowStart }) {
  const { t } = useApp();
  const recent = useRecentFiles();
  // Read again either way: a file that is gone leaves the list.
  const open = (path: string) => showStart.start(() => api.startShowFromRecent(path).finally(recent.reload));
  return (
    <div className="start-pane">
      {showStart.starting ? (
        <p className="muted" role="status">
          {t("quizOpening")}
        </p>
      ) : (
        <p className="muted">{t("welcome")}</p>
      )}
      {recent.files.length > 0 && (
        <>
          <h2>{t("recentFiles")}</h2>
          <RecentFileList files={recent.files} disabled={showStart.starting} onOpen={open} />
        </>
      )}
    </div>
  );
}

/** Starting a show from the Director. */
interface ShowStart {
  /** A start is under way: every way to start one waits for it. */
  starting: boolean;
  /** Runs the start `call` unless one is under way; a failure is reported as `useRun` does. */
  start: (call: () => Promise<unknown>) => void;
}

/**
 * One show start at a time, so a double click opens a quiz once. The
 * backend also refuses a second quiz file start meanwhile (`showStarting`),
 * which needs no word here.
 */
function useShowStart(): ShowStart {
  const run = useRun();
  const [starting, setStarting] = useState(false);
  // Also a ref: two clicks before the next render would both see `starting` false.
  const busy = useRef(false);
  const start = useCallback(
    (call: () => Promise<unknown>) => {
      if (busy.current) return;
      busy.current = true;
      setStarting(true);
      const settled = () => {
        busy.current = false;
        setStarting(false);
      };
      run(call().catch(quietWhileStarting).finally(settled));
    },
    [run],
  );
  return { starting, start };
}

/** Throws `error` on, unless the backend refused a start because another is under way. */
function quietWhileStarting(error: unknown) {
  if (!isAppError(error) || error.code !== "showStarting") throw error;
}

function CommandButton(props: { icon: ReactNode; label: string; hint?: string; tone?: "good" | "bad"; disabled?: boolean; onClick: () => void }) {
  return (
    <button className={`command ${props.tone ?? ""}`} disabled={props.disabled} onClick={props.onClick}>
      <span className="command-icon">{props.icon}</span>
      <span className="command-label">
        {props.label}
        {props.hint && <small>{props.hint}</small>}
      </span>
    </button>
  );
}

/**
 * The step the show takes by itself once everyone answered, with a bar
 * that runs out when it comes; the quizmaster can take a step first or
 * hold it.
 */
function AutoStepNote({ step }: { step: AutoStepView }) {
  const { t } = useApp();
  return (
    <div className="auto-step" role="status">
      {t(step.action === "nextSlide" ? "autoStepNextSlide" : "autoStepReveal")}
      <span key={step.inMs} className="auto-step-bar" style={{ animationDuration: `${step.inMs}ms` }} />
    </div>
  );
}

/** Left command area: only the commands that make sense right now. */
function CommandPanel(props: {
  view: GameView | null;
  projector: ProjectorSettings;
  sound: SlideSound;
  showStart: ShowStart;
  testRun: boolean;
  onEditor: () => void;
}) {
  const { view, projector, sound, showStart, testRun, onEditor } = props;
  const { t, notify } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const last = useLastSession();
  // Why Resume failed, kept on screen until another show loads: the show
  // waits on it.
  const [resumeProblem, setResumeProblem] = useState<unknown>(null);
  useBackendEvent("show-changed", () => setResumeProblem(null));
  const resume = () =>
    showStart.start(() => {
      setResumeProblem(null);
      return api.resumeLastSession().then((warnings) => warnings.length > 0 && notify(warnings.map((w) => errorText(t, w)).join(" ")), setResumeProblem);
    });
  const problem = resumeProblem ?? last.problem;

  const openQuiz = () => showStart.start(() => api.startShowFromFile(quizPicker(t)));

  const controls = view?.controls;
  const latestView = useLatest(view);
  const hint = (key: string) => keyHint(t, key);
  // Asked first, with the clock paused meanwhile so time cannot run out
  // while the quizmaster decides; the answer acts on the show as it is by
  // then (`restartOutcome`).
  const restartQuestion = () => {
    if (!view) return;
    const asked = { slideIndex: view.slideIndex, pausedClock: view.controls.pauseClock };
    if (asked.pausedClock) dispatch({ type: "pauseClock" });
    const answered = (yes: boolean) => {
      const outcome = restartOutcome(yes, asked, latestView.current);
      if (outcome && "command" in outcome) dispatch(outcome.command);
      else if (outcome) notify(t(outcome.notice));
    };
    run(confirmAction(t, t("restartQuestionConfirm")).then(answered));
  };

  return (
    <aside className="command-panel">
      {!view ? (
        <>
          <CommandButton icon={<FolderIcon />} label={t("openQuiz")} disabled={showStart.starting} onClick={openQuiz} />
          <CommandButton icon={<PlayIcon />} label={t("demoQuiz")} disabled={showStart.starting} onClick={() => showStart.start(api.startDemoShow)} />
          <CommandButton icon={<EditIcon />} label={t("quizEditor")} onClick={onEditor} />
          {last.session && (
            <CommandButton
              icon={<NextIcon />}
              label={t("resume")}
              hint={t("resumeInfo", { title: last.session.meta.quizTitle, count: last.session.commandCount })}
              disabled={showStart.starting}
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
          {view.autoStep && <AutoStepNote step={view.autoStep} />}
          {controls?.next && <CommandButton icon={<NextIcon />} label={t("next")} hint={hint(KEYS.next)} onClick={() => dispatch({ type: "next" })} />}
          {controls?.judge && (
            <>
              <CommandButton icon={<CheckIcon />} label={t("right")} hint={hint(KEYS.right)} tone="good" onClick={() => dispatch({ type: "judgeRight" })} />
              <CommandButton icon={<CrossIcon />} label={t("wrong")} hint={hint(KEYS.wrong)} tone="bad" onClick={() => dispatch({ type: "judgeWrong" })} />
            </>
          )}
          {controls?.pauseClock && (
            <CommandButton
              icon={<PauseIcon />}
              label={t(view.autoStep ? "holdAutoStep" : "pause")}
              hint={hint(KEYS.clock)}
              onClick={() => dispatch({ type: "pauseClock" })}
            />
          )}
          {controls?.resumeClock && (
            <CommandButton icon={<PlayIcon />} label={t("resumeClock")} hint={hint(KEYS.clock)} onClick={() => dispatch({ type: "resumeClock" })} />
          )}
          {controls?.closeQuestion && (
            <CommandButton icon={<StopIcon />} label={t("endQuestion")} onClick={() => dispatch({ type: "closeQuestion" })} />
          )}
          {controls?.restartQuestion && (
            <CommandButton
              icon={<RewindIcon />}
              label={t("restartQuestion")}
              onClick={restartQuestion}
            />
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
            onClick={() => run(confirmAction(t, t("endShowConfirm")).then((yes) => (yes ? api.endShow() : undefined)))}
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
