import { useApp } from "../app-context";
import { BackIcon } from "../icons";
import { keyHint, KEYS } from "../keyboard";
import { problemText } from "../messages";
import { MAX_AFTER_REVEAL_DELAY_S, MAX_ALL_ANSWERED_DELAY_S, MAX_JOKERS } from "../bindings/Constants";
import type { StringKey } from "../i18n";
import { questionCount, settingsOf } from "../quiz";
import { RecentFileList } from "../recent-files";
import type { AllAnswered, Quiz, QuizSettings, ValidationError } from "../types";
import type { Action, Actions } from "./actions";
import { NumberBox, Select, Switch } from "./controls";
import type { OpenQuiz } from "./editor-state";

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
   * The file's actions, questions from a spreadsheet, the template for one and the quiz as one (spec QZ-8),
   * and what AI assistants write quizzes with.
   */
  actions: Pick<
    Actions,
    "newQuiz" | "save" | "saveAs" | "closeQuiz" | "exit" | "importSheet" | "sheetTemplate" | "exportSheet" | "saveAiSkill" | "connectClaude" | "connectClaudeFolder"
  >;
  recentFiles: string[];
  /** Opens `recent`, a file in `recentFiles`, or one the quizmaster picks. */
  onOpenFile: (recent?: string) => void;
  onTitle: (title: string) => void;
  onSettings: (settings: QuizSettings) => void;
  /** Shows a slide (by id) in the editor. */
  onShowSlide: (id: string) => void;
}

/** Pages shown when no quiz is open. */
const WITHOUT_QUIZ: BackstagePage[] = ["open", "ai"];

/** The FILE tab: the quiz as a whole and its file, over the whole window. */
export function Backstage(props: BackstageProps) {
  const { open, page, onPage, onClose, actions } = props;
  const { t } = useApp();
  const shown = open || WITHOUT_QUIZ.includes(page) ? page : "open";
  const item = (action: Action) => (
    <button type="button" className="backstage-item" disabled={action.disabled} onClick={action.run}>
      {action.label}
    </button>
  );
  const pageItem = (id: BackstagePage, label: string) => (
    <button type="button" className={shown === id ? "backstage-item active" : "backstage-item"} disabled={!open && !WITHOUT_QUIZ.includes(id)} onClick={() => onPage(id)}>
      {label}
    </button>
  );

  return (
    <div className="backstage">
      <nav className="backstage-menu">
        {onClose && (
          <button type="button" className="backstage-back" aria-label={t("backToQuiz")} title={t("backToQuiz")} onClick={onClose}>
            <BackIcon />
          </button>
        )}
        {pageItem("info", t("info"))}
        {pageItem("settings", t("quizSettings"))}
        {item(actions.newQuiz)}
        {pageItem("open", t("openFile"))}
        {item(actions.save)}
        {item(actions.saveAs)}
        {pageItem("import", t("backstageImport"))}
        {pageItem("export", t("backstageExport"))}
        {pageItem("ai", t("backstageAi"))}
        {item(actions.closeQuiz)}
        <span className="backstage-gap" />
        {item(actions.exit)}
      </nav>
      <div className="backstage-page">{open ? pageFor(shown, props, open) : shown === "ai" ? <AiPage {...props} /> : <OpenPage {...props} />}</div>
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
      return <AiPage {...props} />;
  }
}

function InfoPage({ open, unsaved, onTitle, onShowSlide }: BackstageProps & { open: OpenQuiz }) {
  const { t } = useApp();
  const quiz = open.history.present;
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
      <h2>{t("problems", { n: open.problems.length })}</h2>
      {open.problems.length === 0 ? (
        <p className="good-text">{t("noProblems")}</p>
      ) : (
        <ProblemList problems={open.problems} quiz={quiz} onShowSlide={onShowSlide} />
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
function SettingsPage({ open, onSettings }: BackstageProps & { open: OpenQuiz }) {
  const { t } = useApp();
  const settings = settingsOf(open.history.present);
  const set = (change: Partial<QuizSettings>) => onSettings({ ...settings, ...change });
  const options = (Object.keys(ALL_ANSWERED_LABEL) as AllAnswered[]).map((value): [AllAnswered, string] => [value, t(ALL_ANSWERED_LABEL[value])]);
  const hold = { key: keyHint(t, KEYS.clock) };
  return (
    <>
      <h1>{t("quizSettings")}</h1>
      <p className="backstage-hint">{t("quizSettingsHint")}</p>
      <h2>{t("advancedSettings")}</h2>
      <h3>{t("settingsFlow")}</h3>
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
    </>
  );
}

/** A setting that is on or off, with what it does. */
function SettingSwitch(props: { label: string; hint: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="setting-switch">
      <span>{props.label}</span>
      <Switch label={props.label} checked={props.checked} onChange={props.onChange} />
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

function AiPage({ actions }: BackstageProps) {
  const { t } = useApp();
  return (
    <>
      <h1>{t("backstageAi")}</h1>
      <p>{t("aiAssistantsHint")}</p>
      <div className="backstage-task">
        <div className="backstage-buttons">
          <button type="button" className="browse" onClick={actions.connectClaude.run}>
            {actions.connectClaude.label}
          </button>
          <button type="button" className="browse" onClick={actions.connectClaudeFolder.run}>
            {actions.connectClaudeFolder.label}
          </button>
        </div>
        <p>{t("connectClaudeHint")}</p>
      </div>
      <div className="backstage-task">
        <button type="button" className="browse" onClick={actions.saveAiSkill.run}>
          {actions.saveAiSkill.label}
        </button>
        <p>{t("saveAiSkillHint")}</p>
      </div>
    </>
  );
}

function OpenPage({ recentFiles, onOpenFile }: BackstageProps) {
  const { t } = useApp();
  return (
    <>
      <h1>{t("openFile")}</h1>
      <button type="button" className="browse" onClick={() => onOpenFile()}>
        {t("browse")}
      </button>
      <h2>{t("recentFiles")}</h2>
      {recentFiles.length === 0 ? (
        <p className="muted">{t("noRecentFiles")}</p>
      ) : (
        <RecentFileList files={recentFiles} onOpen={onOpenFile} />
      )}
    </>
  );
}
