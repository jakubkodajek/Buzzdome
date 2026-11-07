import { useApp, useRun } from "../app-context";
import { useVoiceGain, useVoiceModel, useVoicePreview, useVoiceSample, voiceLine, voiceName, VOICES } from "../audio/read-aloud";
import { welcomePreviewView } from "../host/preview";
import { SlideScreen } from "../host/SlideScreen";
import { Stage } from "../host/Stage";
import { useEffect, useId, useRef, useState } from "react";
import { BackIcon, CheckIcon, PlayIcon, StopIcon } from "../icons";
import { keyHint, KEYS } from "../keyboard";
import { problemText } from "../messages";
import { MAX_AFTER_REVEAL_DELAY_S, MAX_ALL_ANSWERED_DELAY_S, MAX_JOKERS, QUIZ_LANGUAGES } from "../bindings/Constants";
import type { StringKey } from "../i18n";
import type { MediaKind } from "../media";
import { LANGUAGE_LABEL, NO_CONTEXT, questionCount, settingsOf, soundsOf, welcomeOf } from "../quiz";
import { RecentFileList } from "../recent-files";
import type { AllAnswered, Quiz, QuizLanguage, QuizSettings, QuizSounds, ReadAloud, ValidationError, Welcome } from "../types";
import { actionButton, tooltip, type Action, type Actions } from "./actions";
import { AiPage } from "./AiPage";
import { MediaPicker, NumberBox, Select, Switch } from "./controls";
import { splitProblems, type OpenQuiz } from "./editor-state";
import { SoundsSettings } from "./SoundsSettings";

export type BackstagePage = "info" | "settings" | "open" | "import" | "export" | "ai";

interface BackstageProps {
  /** The open quiz; null when none is open. */
  open: OpenQuiz | null;
  unsaved: boolean;
  page: BackstagePage;
  onPage: (page: BackstagePage) => void;
  /** Back to the quiz; null when there is no quiz to go back to. */
  onClose: (() => void) | null;
  /**
   * The file's actions, questions from a spreadsheet, the template for one and the quiz as one (spec QZ-8).
   */
  actions: Pick<Actions, "newQuiz" | "openQuiz" | "save" | "saveAs" | "closeQuiz" | "exit" | "importSheet" | "sheetTemplate" | "exportSheet">;
  recentFiles: string[];
  /** Opens `recent`, a file in `recentFiles`, or one the quizmaster picks. */
  onOpenFile: (recent?: string) => void;
  /** Takes a file off `recentFiles`. */
  onForgetFile: (recent: string) => void;
  onTitle: (title: string) => void;
  onSettings: (settings: QuizSettings) => void;
  onSounds: (sounds: QuizSounds) => void;
  onWelcome: (welcome: Welcome) => void;
  /** Lets the quizmaster pick a media file of `kinds`, added to the quiz's media; null when none is picked. */
  onPickMedia: (kinds: MediaKind[]) => Promise<string | null>;
  /** Shows a slide (by id) in the editor. */
  onShowSlide: (id: string) => void;
}

/** Pages shown when no quiz is open. */
const WITHOUT_QUIZ: BackstagePage[] = ["open", "ai"];

/** Back to the quiz, as Escape goes there. */
const BACK_KEYS = ["Escape"];

/** The FILE tab: the quiz as a whole and its file, over the whole window. */
export function Backstage(props: BackstageProps) {
  const { open, page, onPage, onClose, actions } = props;
  const { t } = useApp();
  const shown = open || WITHOUT_QUIZ.includes(page) ? page : "open";
  const menu = useRef<HTMLElement>(null);
  useEffect(() => {
    // Opened by a key, the menu takes the focus, so Tab and Enter go on from the page shown; not from a field or dialog that has it.
    if (document.activeElement === document.body) menu.current?.querySelector<HTMLElement>(".backstage-item.active")?.focus();
  }, []);
  // Each item's tooltip says its keys, as the quick-access buttons' do.
  const item = (action: Action) => (
    <button type="button" className="backstage-item" {...actionButton(action)}>
      {action.label}
    </button>
  );
  const pageItem = (id: BackstagePage, label: string, keys?: string[]) => (
    <button
      type="button"
      className={shown === id ? "backstage-item active" : "backstage-item"}
      title={tooltip({ label, keys })}
      disabled={!open && !WITHOUT_QUIZ.includes(id)}
      onClick={() => onPage(id)}
    >
      {label}
    </button>
  );

  return (
    <div className="backstage">
      <nav className="backstage-menu" ref={menu}>
        {onClose && (
          <button type="button" className="backstage-back" aria-label={t("backToQuiz")} title={tooltip({ label: t("backToQuiz"), keys: BACK_KEYS })} onClick={onClose}>
            <BackIcon />
          </button>
        )}
        {pageItem("info", t("info"))}
        {pageItem("settings", t("quizSettings"))}
        {item(actions.newQuiz)}
        {pageItem("open", actions.openQuiz.label, actions.openQuiz.keys)}
        {item(actions.save)}
        {item(actions.saveAs)}
        {pageItem("import", t("backstageImport"))}
        {pageItem("export", t("backstageExport"))}
        {pageItem("ai", t("backstageAi"))}
        {item(actions.closeQuiz)}
        <span className="backstage-gap" />
        {item(actions.exit)}
      </nav>
      <div className="backstage-page">{open ? pageFor(shown, props, open) : shown === "ai" ? <AiPage /> : <OpenPage {...props} />}</div>
    </div>
  );
}

function pageFor(page: BackstagePage, props: BackstageProps, open: OpenQuiz) {
  switch (page) {
    case "info":
      return <InfoPage {...props} open={open} />;
    case "settings":
      return <SettingsPage {...props} open={open} />;
    case "open":
      return <OpenPage {...props} />;
    case "import":
      return <ImportPage {...props} />;
    case "export":
      return <ExportPage {...props} />;
    case "ai":
      return <AiPage />;
  }
}

function InfoPage({ open, unsaved, onTitle, onShowSlide }: BackstageProps & { open: OpenQuiz }) {
  const { t } = useApp();
  const quiz = open.history.present;
  const { problems, warnings } = splitProblems(open.problems);
  return (
    <>
      <h1>{t("info")}</h1>
      <label className="field">
        <span>{t("quizTitle")}</span>
        <input value={quiz.title} placeholder={t("untitledQuiz")} onChange={(e) => onTitle(e.target.value)} />
      </label>
      <dl className="info-list">
        <dt>{t("file")}</dt>
        <dd>
          {open.sourcePath ?? t("notSavedYet")}
          {unsaved && <span className="chip warn">{t("unsaved")}</span>}
        </dd>
        <dt>{t("slides")}</dt>
        <dd>{quiz.slides.length}</dd>
        <dt>{t("tabQuestions")}</dt>
        <dd>{questionCount(quiz)}</dd>
      </dl>
      {problems.length === 0 ? (
        <h2 className="good-text">
          <CheckIcon className="inline-icon" /> {t("readyToPlay")}
        </h2>
      ) : (
        <>
          <h2>{t("problems", { n: problems.length })}</h2>
          <ProblemList problems={problems} quiz={quiz} onShowSlide={onShowSlide} />
        </>
      )}
      {warnings.length > 0 && (
        <>
          <h2>{t("warnings", { n: warnings.length })}</h2>
          <p>{t("warningsHint")}</p>
          <ProblemList problems={warnings} quiz={quiz} onShowSlide={onShowSlide} />
        </>
      )}
      {open.dropped.length > 0 && (
        <>
          <h2>{t("leftOut", { n: open.dropped.length })}</h2>
          <p>{t("leftOutHint")}</p>
          <ProblemList problems={open.dropped} quiz={quiz} onShowSlide={onShowSlide} />
        </>
      )}
    </>
  );
}

/** What each step the show can take once everyone answered is called. */
const ALL_ANSWERED_LABEL: Record<AllAnswered, StringKey> = {
  wait: "allAnsweredWait",
  reveal: "allAnsweredReveal",
  nextSlide: "allAnsweredNextSlide",
};

/** How the show runs, for the whole quiz (spec QZ-11); each question has its own time, points and answers. */
function SettingsPage({ open, onSettings, onSounds, onWelcome, onPickMedia }: BackstageProps & { open: OpenQuiz }) {
  const { t } = useApp();
  const quiz = open.history.present;
  const settings = settingsOf(quiz);
  const set = (change: Partial<QuizSettings>) => onSettings({ ...settings, ...change });
  const options = (Object.keys(ALL_ANSWERED_LABEL) as AllAnswered[]).map((value): [AllAnswered, string] => [value, t(ALL_ANSWERED_LABEL[value])]);
  const hold = { key: keyHint(t, KEYS.clock) };
  return (
    <>
      <h1>{t("quizSettings")}</h1>
      <p className="backstage-hint">{t("quizSettingsHint")}</p>
      <h3>{t("settingsFlow")}</h3>
      <SettingSwitch label={t("startOnNext")} hint={t("startOnNextHint")} checked={settings.startOnNext} onChange={(startOnNext) => set({ startOnNext })} />
      <div className="field">
        <span>{t("allAnswered")}</span>
        <Select label={t("allAnswered")} value={settings.allAnswered} options={options} onChange={(allAnswered) => set({ allAnswered })} />
      </div>
      {settings.allAnswered !== "wait" && (
        <div className="field">
          <span>{t("allAnsweredDelay")}</span>
          <NumberBox
            label={t("allAnsweredDelay")}
            value={settings.allAnsweredDelayS}
            min={0}
            max={MAX_ALL_ANSWERED_DELAY_S}
            unit="s"
            onChange={(allAnsweredDelayS) => set({ allAnsweredDelayS })}
          />
        </div>
      )}
      <p className="backstage-hint">{t("allAnsweredHint", hold)}</p>
      <SettingSwitch label={t("runClockOut")} hint={t("runClockOutHint")} checked={settings.runClockOut} onChange={(runClockOut) => set({ runClockOut })} />
      <SettingSwitch
        label={t("nextAfterReveal")}
        hint={t("nextAfterRevealHint", hold)}
        checked={settings.nextAfterReveal}
        onChange={(nextAfterReveal) => set({ nextAfterReveal })}
      />
      {settings.nextAfterReveal && (
        <div className="field">
          <span>{t("nextAfterRevealDelay")}</span>
          <NumberBox
            label={t("nextAfterRevealDelay")}
            value={settings.nextAfterRevealDelayS}
            min={0}
            max={MAX_AFTER_REVEAL_DELAY_S}
            unit="s"
            onChange={(nextAfterRevealDelayS) => set({ nextAfterRevealDelayS })}
          />
        </div>
      )}
      <h3>{t("settingsAnswers")}</h3>
      <SettingSwitch label={t("changeAnswer")} hint={t("changeAnswerHint")} checked={settings.changeAnswer} onChange={(changeAnswer) => set({ changeAnswer })} />
      <SettingSwitch label={t("oneBuzz")} hint={t("oneBuzzHint")} checked={settings.oneBuzz} onChange={(oneBuzz) => set({ oneBuzz })} />
      <SettingSwitch
        label={t("noAnswerPenalty")}
        hint={t("noAnswerPenaltyHint")}
        checked={settings.noAnswerPenalty}
        onChange={(noAnswerPenalty) => set({ noAnswerPenalty })}
      />
      <div className="field">
        <span>{t("jokers")}</span>
        <NumberBox label={t("jokers")} value={settings.jokers} min={0} max={MAX_JOKERS} onChange={(jokers) => set({ jokers })} />
      </div>
      <p className="backstage-hint">{t("jokersHint")}</p>
      <h3>{t("settingsProjector")}</h3>
      <SettingSwitch label={t("showAnswer")} hint={t("showAnswerHint")} checked={!settings.hideAnswer} onChange={(show) => set({ hideAnswer: !show })} />
      <SettingSwitch
        label={t("showBuzzPoints")}
        hint={t("showBuzzPointsHint")}
        checked={!settings.hideBuzzPoints}
        onChange={(show) => set({ hideBuzzPoints: !show })}
      />
      <SettingSwitch label={t("answerChart")} hint={t("answerChartHint")} checked={settings.answerChart} onChange={(answerChart) => set({ answerChart })} />
      <ReadAloudSettings quiz={quiz} settings={settings} set={set} />
      <WelcomeSettings quiz={quiz} onWelcome={onWelcome} onPickMedia={onPickMedia} />
      <SoundsSettings sounds={soundsOf(quiz)} onSounds={onSounds} onPickFile={() => onPickMedia(["audio"])} />
    </>
  );
}

/** What of a question can be read out. */
const READ_ALOUD_LABEL: Record<ReadAloud, StringKey> = { off: "readAloudOff", question: "readAloudQuestion", questionAndAnswers: "readAloudQuestionAndAnswers" };

/** Reading questions aloud (spec SN-10): what is read, in which language, by which voice. */
function ReadAloudSettings({ quiz, settings, set }: { quiz: Quiz; settings: QuizSettings; set: (change: Partial<QuizSettings>) => void }) {
  const { t } = useApp();
  const run = useRun();
  const model = useVoiceModel();
  const preview = useVoicePreview();
  const sample = useVoiceSample();
  const gain = useVoiceGain(quiz);
  const reads = (Object.keys(READ_ALOUD_LABEL) as ReadAloud[]).map((value): [ReadAloud, string] => [value, t(READ_ALOUD_LABEL[value])]);
  const languages = QUIZ_LANGUAGES.map((value): [QuizLanguage, string] => [value, t(LANGUAGE_LABEL[value])]);
  const voices = VOICES.map((value): [number, string] => [value, voiceName(t, value)]);
  const trying = preview.playing || sample.playing;
  const [preparing, setPreparing] = useState(false);
  const tryVoice = () => {
    if (trying) {
      preview.stop();
      return sample.stop();
    }
    if (!model?.installed) return sample.play(settings.language, settings.voice);
    // The first question, as the show reads it; read in full even while reading is off.
    const what = settings.readAloud === "off" ? "questionAndAnswers" : settings.readAloud;
    const line = quiz.slides.map((slide) => voiceLine(slide, settings, what)).find((found) => found !== null);
    if (!line) return sample.play(settings.language, settings.voice);
    // The clip renders for a few seconds before it plays (or is stopped): that wait is shown.
    setPreparing(true);
    run(preview.play(line, gain).finally(() => setPreparing(false)));
  };
  return (
    <>
      <h3>{t("settingsVoice")}</h3>
      <div className="field">
        <span>{t("readAloud")}</span>
        <Select label={t("readAloud")} value={settings.readAloud} options={reads} onChange={(readAloud) => set({ readAloud })} />
      </div>
      <p className="backstage-hint">{t("readAloudHint")}</p>
      {settings.readAloud !== "off" && !settings.startOnNext && (
        <SettingSwitch
          label={t("clockAfterReading")}
          hint={t("clockAfterReadingHint")}
          checked={settings.clockAfterReading}
          onChange={(clockAfterReading) => set({ clockAfterReading })}
        />
      )}
      <div className="field">
        <span>{t("quizLanguage")}</span>
        <Select label={t("quizLanguage")} value={settings.language} options={languages} onChange={(language) => set({ language })} />
      </div>
      <p className="backstage-hint">{t("quizLanguageHint")}</p>
      <div className="field">
        <span>{t("voiceChoice")}</span>
        <div className="voice-choice">
          <Select label={t("voiceChoice")} value={settings.voice} options={voices} onChange={(voice) => set({ voice })} />
          <button
            type="button"
            className={preparing ? "mini preparing" : "mini"}
            aria-label={t(trying ? "stopTryVoice" : "tryVoice")}
            aria-busy={preparing}
            title={t(trying ? "stopTryVoice" : "tryVoice")}
            onClick={tryVoice}
          >
            {trying ? <StopIcon /> : <PlayIcon />}
          </button>
          {preparing && (
            <span className="voice-preparing" role="status">
              {t("voicePreparing")}
            </span>
          )}
        </div>
      </div>
      {model && !model.installed && <p className="backstage-hint">{t("voiceNotDownloaded")}</p>}
    </>
  );
}

/** The welcome screen the show opens on (spec SF-3), drawn as the projector will show it. */
function WelcomeSettings({ quiz, onWelcome, onPickMedia }: { quiz: Quiz; onWelcome: (welcome: Welcome) => void; onPickMedia: (kinds: MediaKind[]) => Promise<string | null> }) {
  const { t } = useApp();
  const welcome = welcomeOf(quiz);
  const set = (change: Partial<Welcome>) => onWelcome({ ...welcome, ...change });
  const text = (label: string, value: string, change: (text: string) => Partial<Welcome>, placeholder?: string) => (
    <label>
      <span>{label}</span>
      <input value={value} placeholder={placeholder} onChange={(e) => set(change(e.target.value))} />
    </label>
  );
  const file = (label: string, value: string | null, kinds: MediaKind[], change: (file: string | null) => Partial<Welcome>) => (
    <>
      <span>{label}</span>
      <MediaPicker label={label} file={value} onChoose={() => void onPickMedia(kinds).then((picked) => picked && set(change(picked)))} onRemove={() => set(change(null))} />
    </>
  );
  const view = welcomePreviewView(welcome, quiz.title, quiz.slides.length);
  return (
    <>
      <h3>{t("welcomeScreen")}</h3>
      <SettingSwitch label={t("welcomeOn")} hint={t("welcomeOnHint", { key: keyHint(t, KEYS.next) })} checked={welcome.on} onChange={(on) => set({ on })} />
      <div className="welcome-fields">
        {text(t("welcomeHeading"), welcome.heading, (heading) => ({ heading }), quiz.title)}
        {file(t("welcomeLogo"), welcome.logo, ["image"], (logo) => ({ logo }))}
        {file(t("welcomeIntro"), welcome.intro, ["image", "video"], (intro) => ({ intro }))}
        {text(t("welcomeFooter"), welcome.footer, (footer) => ({ footer }))}
      </div>
      <p className="backstage-hint">{t("welcomeIntroHint")}</p>
      <Stage className="thumbnail-stage welcome-preview" screen="welcome" slide={null} mode="thumbnail">
        <SlideScreen view={view} context={NO_CONTEXT} clock={null} mode="thumbnail" />
      </Stage>
    </>
  );
}

/** A setting that is on or off, with what it does; a click on its name switches it too. */
function SettingSwitch(props: { label: string; hint: string; checked: boolean; onChange: (on: boolean) => void }) {
  const id = useId();
  return (
    <div className="setting-switch">
      <label htmlFor={id}>{props.label}</label>
      <Switch id={id} label={props.label} checked={props.checked} onChange={props.onChange} />
      <p className="backstage-hint">{props.hint}</p>
    </div>
  );
}

/** Problems with the slide each is on, which shows that slide. */
function ProblemList({ problems, quiz, onShowSlide }: { problems: ValidationError[]; quiz: Quiz; onShowSlide: (id: string) => void }) {
  const { t } = useApp();
  const slideNumber = new Map(quiz.slides.map((slide, i) => [slide.id, i + 1]));
  return (
    <ul className="problem-list">
      {problems.map((problem, i) => {
        const id = problem.slideId;
        const n = id === null ? undefined : slideNumber.get(id);
        return (
          <li key={i}>
            {id !== null && n !== undefined ? (
              <button type="button" className="link" onClick={() => onShowSlide(id)}>
                {t("slide", { n, total: quiz.slides.length })}
              </button>
            ) : (
              <b>{t("wholeQuiz")}</b>
            )}
            <span>{problemText(t, problem.problem)}</span>
          </li>
        );
      })}
    </ul>
  );
}

function ImportPage({ actions }: BackstageProps) {
  const { t } = useApp();
  return (
    <>
      <h1>{t("backstageImport")}</h1>
      <div className="backstage-task">
        <button type="button" className="browse" disabled={actions.importSheet.disabled} onClick={actions.importSheet.run}>
          {actions.importSheet.label}
        </button>
        <p>{t("importFromSheetHint")}</p>
      </div>
      <div className="backstage-task">
        <button type="button" className="browse" disabled={actions.sheetTemplate.disabled} onClick={actions.sheetTemplate.run}>
          {actions.sheetTemplate.label}
        </button>
        <p>{t("saveSheetTemplateHint")}</p>
      </div>
    </>
  );
}

function ExportPage({ actions }: BackstageProps) {
  const { t } = useApp();
  return (
    <>
      <h1>{t("backstageExport")}</h1>
      <div className="backstage-task">
        <button type="button" className="browse" disabled={actions.exportSheet.disabled} onClick={actions.exportSheet.run}>
          {actions.exportSheet.label}
        </button>
        <p>{t("exportToSheetHint")}</p>
      </div>
    </>
  );
}

function OpenPage({ recentFiles, onOpenFile, onForgetFile, actions }: BackstageProps) {
  const { t } = useApp();
  // Not while the editor works on a file.
  const busy = actions.openQuiz.disabled;
  return (
    <>
      <h1>{t("openFile")}</h1>
      <button type="button" className="browse" disabled={busy} onClick={() => onOpenFile()}>
        {t("browse")}
      </button>
      <h2>{t("recentFiles")}</h2>
      {recentFiles.length === 0 ? (
        <p className="muted">{t("noRecentFiles")}</p>
      ) : (
        <RecentFileList files={recentFiles} disabled={busy} onOpen={onOpenFile} onForget={onForgetFile} />
      )}
    </>
  );
}
