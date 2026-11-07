import { useCallback, useState, type ReactNode } from "react";
import { api } from "../api";
import { AppProvider, useApp, useDispatch, useRun, useStepShown } from "../app-context";
import { otherSoundPlays, useShowSounds } from "../audio/show-sounds";
import { useSlideSound, type SlideSound } from "../audio/slide-sound";
import { usePrepareVoice, useReadAloud, useVoiceSpeaks, type ReadAloudControl } from "../audio/read-aloud";
import { clockSeconds } from "../format";
import { EditorScreen } from "../editor/EditorScreen";
import { ErrorBoundary, RenderFailed } from "../error-boundary";
import { errorNotice, errorText } from "../messages";
import { NoticeBox, NotSavedBanner } from "../notice";
import {
  useBackendEvent,
  useClock,
  useGameView,
  useLatest,
  useLearnProgress,
  useMediaRoot,
  useQuiz,
  useShowNotSaved,
  useWindowTitle,
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
  PreviousIcon,
  RewindIcon,
  ScreenIcon,
  SoundIcon,
  SpeakIcon,
  StopIcon,
  VideoIcon,
} from "../icons";
import { commandForAction, commandForRemote, keyHint, KEYS, projectorKeyActs, useRemote, useShortcuts } from "../keyboard";
import { MediaRootContext } from "../media";
import { beforeSlides, currentSlide, judgedLabel, nextLabel, phaseLabel } from "../phase";
import { slideContext } from "../quiz";
import { RecentFileList } from "../recent-files";
import { ThemeContext, ThemesProvider, useResolvedTheme, useThemes } from "../theme/context";
import { isThemeMissing } from "../theme/themes";
import type { AutoStepView, GameView, Quiz } from "../types";
import { openDialogs } from "../confirm";
import { useStore } from "../store";
import { useHardwareStatus, type HardwareChip } from "./hardware-status";
import { QuestionsTab } from "./QuestionsTab";
import { LaunchCheck } from "./LaunchCheck";
import { useLaunchCheckSetting, type LaunchCheckSetting } from "./launch-check";
import { LightsTab } from "./LightsTab";
import { useDirectorLook, type DirectorLook } from "./look";
import { ReceiverTab } from "./ReceiverTab";
import { MediaPlayerTab } from "./MediaPlayerTab";
import { ResponsesTab } from "./ResponsesTab";
import { ResultsTab } from "./ResultsTab";
import { SettingsTab, useProjectorSettings, type ProjectorSettings } from "./SettingsTab";
import { TeamsTab } from "./TeamsTab";
import { UpdateBanner } from "./UpdateBanner";
import { confirmAction } from "../confirm";
import { useMediaPlayer, type MediaPlayer } from "./useMediaPlayer";
import { restartOutcome } from "./restart";
import { autoStepBar, type AutoStepBar } from "./auto-step";
import { resumeInfo, staleFile, useShowStart, type ShowStart } from "./show-start";
import { ToastStack, useToasts } from "./Toasts";
import { VirtualBuzzers } from "./VirtualBuzzers";
import "./director.css";

/** What the remote does while a system dialog is open: nothing. */
const ignore = () => undefined;

type Tab = "questions" | "teams" | "responses" | "results" | "buzzers" | "media" | "receiver" | "lights" | "settings";

/** The show controls, the quiz editor, or the show controls for a test run of the edited quiz. */
type Screen = "show" | "editor" | "testRun";

/** The quizmaster's window. */
export function Director() {
  const { toasts, show, close } = useToasts();
  return (
    <AppProvider show={show}>
      <ThemesProvider>
        <DirectorScreens />
      </ThemesProvider>
      <ToastStack toasts={toasts} close={close} />
    </AppProvider>
  );
}

function DirectorScreens() {
  const { t, fail } = useApp();
  const [screen, setScreen] = useState<Screen>("show");
  // No show runs without its controls: one that starts while the editor is
  // open (a start under way when it opened) takes the window back to them.
  // Only as it starts: back from a test run the editor opens before the
  // window hears that the show ended.
  const showRuns = useGameView() !== null;
  const [ran, setRan] = useState(showRuns);
  if (showRuns !== ran) {
    setRan(showRuns);
    if (showRuns && screen === "editor") setScreen("show");
  }
  // From the Director's button the editor asks what to work on; back from a test run it does not.
  const [askOnEditor, setAskOnEditor] = useState(false);
  const openEditor = (ask: boolean) => {
    setAskOnEditor(ask);
    setScreen("editor");
  };
  useBackendEvent("host-problem", ({ text, detail }) => fail({ text: t("hostProblem", { message: text }), detail }));
  // Provided above the show controls: their hooks (the slide sound) resolve media too.
  const mediaRoot = useMediaRoot();
  if (screen === "editor") {
    return (
      // An editor that fails to render keeps the way back to the show.
      <ErrorBoundary
        fallback={(message, retry) => (
          <RenderFailed t={t} text={t("partFailed")} detail={message} retry={retry}>
            <button onClick={() => setScreen("show")}>{t("backToDirector")}</button>
          </RenderFailed>
        )}
      >
        <EditorScreen ask={askOnEditor} onTestRun={() => setScreen("testRun")} onExit={() => setScreen("show")} />
      </ErrorBoundary>
    );
  }
  return (
    <MediaRootContext value={mediaRoot}>
      <DirectorLayout testRun={screen === "testRun"} onEditor={openEditor} />
    </MediaRootContext>
  );
}

/**
 * The show controls. In a test run the show ends back in the editor;
 * `onEditor` opens it.
 */
function DirectorLayout({ testRun, onEditor }: { testRun: boolean; onEditor: (ask: boolean) => void }) {
  const { t, caution } = useApp();
  const view = useGameView();
  const quiz = useQuiz();
  useWindowTitle(view ? view.quizTitle : t("groupDirector"), t("appTitle"));
  const theme = useResolvedTheme(quiz?.theme ?? null);
  const projector = useProjectorSettings();
  const look = useDirectorLook();
  const launch = useLaunchCheckSetting();
  const [tab, setTab] = useState<Tab>("questions");
  useStepShown(view);
  // M and the remote's projector key: during a show they only open the projector, and say how to close it.
  const showRuns = view !== null;
  const { isOpen, toggle } = projector;
  const projectorKey = useCallback(
    () => (projectorKeyActs(showRuns, isOpen) ? toggle() : caution(t("projectorKeyNoClose"))),
    [showRuns, isOpen, toggle, caution, t],
  );
  useShortcuts(view, projectorKey);
  // The remote waits while a system dialog asks about the show as it was (`openDialogs`).
  const asking = useStore(openDialogs) > 0;
  useRemote(asking ? null : view, asking ? ignore : projectorKey);
  useBackendEvent("receiver-remote", ({ action }) => {
    if (!view || action === "nothing") return;
    if (openDialogs.get() > 0) caution(t("remoteWhileAsking"));
    // Next that the remote leaves to End question (`commandForRemote`) says so, not nothing.
    else if (action === "next" && commandForAction(action, view) && !commandForRemote(action, view)) caution(t("remoteNextNoEnd"));
  });
  const hardware = useHardwareStatus(showRuns);
  // Sounds play here: this window is open for the whole show, the projector may not be.
  const sound = useSlideSound(view);
  // Here too, so what plays goes on and ends whichever tab is open.
  const player = useMediaPlayer();
  // And the questions read out, with each one's clip rendered beforehand.
  usePrepareVoice(quiz, view);
  const voice = useReadAloud(view, quiz);
  const voiceSpeaks = useVoiceSpeaks();
  useShowSounds(view, quiz, otherSoundPlays(view, sound.playing || voiceSpeaks, player.now));
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
    <div className={look.dark ? "director dark" : "director"} data-step={view?.stepNo}>
      <CommandPanel view={view} projector={projector} sound={sound} voice={voice} showStart={showStart} testRun={testRun} onEditor={onEditor} />
      <div className="workspace">
        <nav className="tabs" role="tablist">
          {tabs.map(([id, label]) => (
            <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "tab active" : "tab"} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </nav>
        <ShowNotSaved />
        <LearnBanner />
        {view && beforeSlides(view.phase) && launch.on && <LaunchCheck view={view} quiz={quiz} projector={projector} onTab={setTab} />}
        <UpdateBanner showRunning={view !== null} />
        <div className="tab-body" role="tabpanel">
          <ThemeContext value={theme}>
            {/* A tab that fails to render keeps the rest of the window, and its keys, working. */}
            <ErrorBoundary key={tab} fallback={(message, retry) => <RenderFailed t={t} text={t("partFailed")} detail={message} retry={retry} />}>
              <TabBody
                tab={tab}
                view={view}
                quiz={quiz}
                projector={projector}
                look={look}
                launch={launch}
                player={player}
                sound={sound}
                showStart={showStart}
                onEditor={onEditor}
              />
            </ErrorBoundary>
          </ThemeContext>
        </div>
        <StatusBar view={view} quiz={quiz} testRun={testRun} projectorOpen={projector.isOpen} hardware={hardware} onTab={setTab} />
      </div>
    </div>
  );
}

/** Shown while the show's journal refuses writes, when nothing in the show changes, or a write does not finish. */
function ShowNotSaved() {
  const { t } = useApp();
  const problem = useShowNotSaved();
  if (!problem) return null;
  const { text, detail } = errorNotice(t, problem);
  // A write that hangs keeps the presses for when it finishes, as its own text says.
  return <NotSavedBanner notice={{ text: problem.code === "saveHangs" ? text : t("notSavedBanner", { problem: text }), detail }} />;
}

/** Why the last show cannot be resumed, in the command panel under its button. */
function ResumeProblem({ problem }: { problem: unknown }) {
  const { t } = useApp();
  const { text, detail } = errorNotice(t, problem);
  return (
    <div className="command-problem">
      <NoticeBox notice={{ text: t("resumeProblem", { problem: text }), detail }} />
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
  look: DirectorLook;
  launch: LaunchCheckSetting;
  player: MediaPlayer;
  sound: SlideSound;
  showStart: ShowStart;
  onEditor: (ask: boolean) => void;
}

function TabBody({ tab, view, quiz, projector, look, launch, player, sound, showStart, onEditor }: TabBodyProps) {
  // Setup tabs work without a show; the others need one.
  if (tab === "settings") return <SettingsTab projector={projector} look={look} launch={launch} showRunning={view !== null} />;
  if (tab === "receiver") return <ReceiverTab />;
  if (tab === "lights") return <LightsTab />;
  // Team names can be prepared before a show, and the media player plays without one.
  if (tab === "teams") return <TeamsTab view={view} />;
  if (tab === "media") return <MediaPlayerTab player={player} sound={sound} view={view} />;
  if (!view) return <StartPane showStart={showStart} onEditor={onEditor} />;
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

/**
 * The show's tabs before a show: the recent quiz files, each starting a show
 * in a click (spec SF-1), and a note while the editor holds changes that a
 * quiz file does not have yet.
 */
function StartPane({ showStart, onEditor }: { showStart: ShowStart; onEditor: (ask: boolean) => void }) {
  const { t } = useApp();
  const run = useRun();
  const { recent } = showStart;
  const stale = staleFile(showStart.draft);
  return (
    <div className="start-pane">
      {showStart.starting ? (
        <p className="muted" role="status">
          {t("quizOpening")}
        </p>
      ) : (
        <p className="muted">{t(showStart.last.session ? "welcome" : "welcomeNoResume")}</p>
      )}
      {stale && (
        <div className="start-stale">
          <NoticeBox tone="warn" notice={{ text: t("editorUnsavedNotice", { title: stale.title }), detail: stale.sourcePath ?? undefined }}>
            <button disabled={showStart.starting} onClick={() => onEditor(false)}>
              {t("backToEditor")}
            </button>
          </NoticeBox>
        </div>
      )}
      {recent.files.length > 0 && (
        <>
          <h2>{t("recentFiles")}</h2>
          <RecentFileList files={recent.files} disabled={showStart.starting} onOpen={showStart.startFile} onForget={(path) => run(recent.forget(path))} />
        </>
      )}
    </div>
  );
}

/** A command; `pressed`: a toggle, pressed while what it shows is on (the ranking on the projector). */
function CommandButton(props: { icon: ReactNode; label: string; hint?: string; tone?: "good" | "bad"; pressed?: boolean; disabled?: boolean; onClick: () => void }) {
  return (
    <button className={`command ${props.tone ?? ""}`} aria-pressed={props.pressed} disabled={props.disabled} onClick={props.onClick}>
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
function AutoStepNote({ view, step }: { view: GameView; step: AutoStepView }) {
  const { t } = useApp();
  const bar = autoStepBar(view);
  return (
    <div className="auto-step" role="status">
      {t(step.action === "nextSlide" ? "autoStepNextSlide" : "autoStepReveal")}
      {bar && <AutoStepRunsOut key={bar.key} bar={bar} />}
    </div>
  );
}

/** The bar of one wait, set as it starts: views during the wait neither restart nor stretch it. */
function AutoStepRunsOut({ bar }: { bar: AutoStepBar }) {
  const [run] = useState(bar);
  return <span className="auto-step-bar" style={{ animationDuration: `${run.totalMs}ms`, animationDelay: `${-run.elapsedMs}ms` }} />;
}

/**
 * Left command area. The step commands keep their places, offered or not
 * (`GameView.controls`), so a buzz never moves another command under the
 * quizmaster's mouse.
 */
function CommandPanel(props: {
  view: GameView | null;
  projector: ProjectorSettings;
  sound: SlideSound;
  voice: ReadAloudControl;
  showStart: ShowStart;
  testRun: boolean;
  /** Opens the quiz editor; `ask`: asking first what to work on. */
  onEditor: (ask: boolean) => void;
}) {
  const { view, projector, sound, voice, showStart, testRun, onEditor } = props;
  const { t, lang, notify, caution } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const { last } = showStart;
  // Why Resume failed, kept on screen until another show loads: the show
  // waits on it.
  const [resumeProblem, setResumeProblem] = useState<unknown>(null);
  useBackendEvent("show-changed", () => setResumeProblem(null));
  const resume = () =>
    showStart.resume(() => {
      setResumeProblem(null);
      return api.resumeLastSession().then((warnings) => warnings.length > 0 && caution(warnings.map((w) => errorText(t, w)).join(" ")), setResumeProblem);
    });
  const problem = resumeProblem ?? last.problem;

  const latestView = useLatest(view);
  const hint = (key: string) => keyHint(t, key);
  const projectorHint = projectorKeyActs(view !== null, projector.isOpen) ? hint(KEYS.projector) : undefined;
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
      else if (outcome) (outcome.tone === "warn" ? caution : notify)(t(outcome.notice));
    };
    run(confirmAction(t, t("restartQuestionConfirm"), t("restartQuestion")).then(answered));
  };

  return (
    // The commands scroll in a short window; the projector and the way out stay at the bottom.
    <aside className="command-panel">
      <div className="command-list">
        {!view ? (
          <>
            {/* First while it is offered: after a crash it is the way back. */}
            {last.session && (
              <CommandButton
                icon={<NextIcon />}
                label={t("resume")}
                hint={resumeInfo(t, lang, last.session)}
                disabled={showStart.starting}
                onClick={resume}
              />
            )}
            {problem !== null && <ResumeProblem problem={problem} />}
            <CommandButton icon={<FolderIcon />} label={t("openQuiz")} disabled={showStart.starting} onClick={() => showStart.startFile()} />
            <CommandButton icon={<PlayIcon />} label={t("demoQuiz")} disabled={showStart.starting} onClick={() => showStart.start(api.startDemoShow)} />
            <CommandButton icon={<EditIcon />} label={t("quizEditor")} disabled={showStart.starting} onClick={() => onEditor(true)} />
          </>
        ) : (
          <>
            <StepCommands view={view} onRestart={restartQuestion} />
            {view.autoStep && <AutoStepNote view={view} step={view.autoStep} />}
            {sound.available &&
              (sound.playing ? (
                <CommandButton icon={<StopIcon />} label={t("stopSound")} onClick={sound.stop} />
              ) : (
                <CommandButton icon={<SoundIcon />} label={t("playSound")} onClick={sound.play} />
              ))}
            {voice.available &&
              (voice.speaking ? (
                <CommandButton icon={<StopIcon />} label={t("stopReading")} onClick={voice.stop} />
              ) : (
                <CommandButton icon={<SpeakIcon />} label={t("readQuestion")} onClick={voice.read} />
              ))}
            {view.controls.playVideo && <CommandButton icon={<VideoIcon />} label={t("playVideo")} onClick={() => dispatch({ type: "playVideo" })} />}
            <CommandButton
              icon={<ListIcon />}
              label={t(view.leaderboardVisible ? "leaderboardHide" : "leaderboardShow")}
              hint={hint(KEYS.leaderboard)}
              pressed={view.leaderboardVisible}
              onClick={() => dispatch({ type: "showLeaderboard", visible: !view.leaderboardVisible })}
            />
          </>
        )}
      </div>
      <div className="command-footer">
        <CommandButton
          icon={<ScreenIcon />}
          label={t(projector.isOpen ? "closeProjectorWindow" : "openProjectorWindow")}
          hint={projectorHint}
          onClick={projector.toggle}
        />
        {view &&
          (testRun ? (
            <CommandButton icon={<BackIcon />} label={t("backToEditor")} onClick={() => run(api.endShow().then(() => onEditor(false)))} />
          ) : (
            <CommandButton
              icon={<PowerIcon />}
              label={t("endShow")}
              tone="bad"
              onClick={() => run(confirmAction(t, t("endShowConfirm"), t("endShow")).then((yes) => (yes ? api.endShow() : undefined)))}
            />
          ))}
      </div>
    </aside>
  );
}

/**
 * The step commands, each in its place whether the step offers it or not:
 * Next, who is judged, Right and Wrong, the clock, End and Restart
 * question, and back a slide.
 */
function StepCommands({ view, onRestart }: { view: GameView; onRestart: () => void }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const { controls } = view;
  const hint = (key: string) => keyHint(t, key);
  const judged = judgedLabel(t, view);
  const previous = commandForAction("previous", view);
  // One place for the clock: Resume while it is paused, else Pause (or hold the step the show takes by itself).
  const clock = controls.resumeClock
    ? { icon: <PlayIcon />, label: t("resumeClock"), command: { type: "resumeClock" } as const }
    : { icon: <PauseIcon />, label: t(view.autoStep ? "holdAutoStep" : "pause"), command: { type: "pauseClock" } as const };
  return (
    <>
      {/* Says what Next does now; "Next" and its key stay beside it. */}
      <CommandButton
        icon={<NextIcon />}
        label={nextLabel(t, view)}
        hint={controls.nextStep ? `${t("next")} · ${hint(KEYS.next)}` : hint(KEYS.next)}
        disabled={!controls.next}
        onClick={() => dispatch({ type: "next" })}
      />
      {/* Always there, two lines high, so a buzz does not move the commands under it: who, then the queue and buzzer. */}
      <div className="judged" role="status" title={judged ? [judged.who, judged.detail].filter(Boolean).join(" · ") : undefined}>
        {judged && (
          <>
            <span>{judged.who}</span>
            {judged.detail && <span>{judged.detail}</span>}
          </>
        )}
      </div>
      <CommandButton icon={<CheckIcon />} label={t("right")} hint={hint(KEYS.right)} tone="good" disabled={!controls.judge} onClick={() => dispatch({ type: "judgeRight" })} />
      <CommandButton icon={<CrossIcon />} label={t("wrong")} hint={hint(KEYS.wrong)} tone="bad" disabled={!controls.judge} onClick={() => dispatch({ type: "judgeWrong" })} />
      <CommandButton
        icon={clock.icon}
        label={clock.label}
        hint={hint(KEYS.clock)}
        disabled={!controls.pauseClock && !controls.resumeClock}
        onClick={() => dispatch(clock.command)}
      />
      <CommandButton icon={<StopIcon />} label={t("endQuestion")} disabled={!controls.closeQuestion} onClick={() => dispatch({ type: "closeQuestion" })} />
      <CommandButton icon={<RewindIcon />} label={t("restartQuestion")} disabled={!controls.restartQuestion} onClick={onRestart} />
      <CommandButton icon={<PreviousIcon />} label={t("previousSlide")} hint={hint(KEYS.previous)} disabled={!previous} onClick={() => previous && dispatch(previous)} />
    </>
  );
}

interface StatusBarProps {
  view: GameView | null;
  quiz: Quiz | null;
  testRun: boolean;
  projectorOpen: boolean;
  /** The receiver and lights as the hall may notice them, each opening the tab that puts it right. */
  hardware: HardwareChip[];
  onTab: (tab: Tab) => void;
}

function StatusBar({ view, quiz, testRun, projectorOpen, hardware, onTab }: StatusBarProps) {
  const { t } = useApp();
  const clock = useClock(view);
  const themes = useThemes();
  if (!view) return <footer className="status-bar">{t("noShow")}</footer>;
  // No slide number while none is on the projector (registration, the ranking after the last slide).
  const current = currentSlide(view);
  const position = current === null ? null : slideContext(quiz, current).position;
  return (
    <footer className="status-bar">
      <span className="status-title" title={view.quizTitle}>
        {view.quizTitle}
      </span>
      {current !== null && <span>{t("slide", { n: current + 1, total: view.slideCount })}</span>}
      {position && (
        <span>
          {t("colQuestionNo")} {position.n}/{position.total}
        </span>
      )}
      <span className="status-phase">{phaseLabel(t, view)}</span>
      {clock && <span className="status-clock">{clock.remainingMs === null ? t("untimed") : t("clockLeft", { s: clockSeconds(clock.remainingMs) })}</span>}
      {view.leaderboardVisible && <span className="status-chip">{t("leaderboardOnScreen")}</span>}
      {!projectorOpen && <span className="status-chip">{t("projectorClosed")}</span>}
      {hardware.map((chip) => (
        <button
          key={chip.label}
          type="button"
          className={chip.tone === "warn" ? "status-warning" : "status-chip"}
          title={[chip.detail, t("launchGoTo", { tab: t(chip.tab === "receiver" ? "tabReceiver" : "tabLights") })].filter(Boolean).join(" ")}
          onClick={() => onTab(chip.tab)}
        >
          {t(chip.label)}
        </button>
      ))}
      <span className="spacer" />
      {quiz?.theme && isThemeMissing(themes, quiz.theme) && <span className="status-warning">{t("themeNotInstalled", { id: quiz.theme })}</span>}
      {testRun && <span className="status-test-run">{t("testRunActive")}</span>}
      <span>{t("registered", { n: view.players.length })}</span>
    </footer>
  );
}
