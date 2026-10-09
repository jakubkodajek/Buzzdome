import { useApp } from "../app-context";
import { BackIcon } from "../icons";
import { questionCount } from "../quiz";
import type { Action } from "./actions";
import type { OpenQuiz } from "./editor-state";

export type BackstagePage = "info" | "open";

interface BackstageProps {
  /** The open quiz; null when none is open. */
  open: OpenQuiz | null;
  unsaved: boolean;
  page: BackstagePage;
  onPage: (page: BackstagePage) => void;
  /** Back to the quiz; null when there is no quiz to go back to. */
  onClose: (() => void) | null;
  actions: { newQuiz: Action; save: Action; saveAs: Action; closeQuiz: Action; exit: Action };
  recentFiles: string[];
  onOpenFile: (path?: string) => void;
  onTitle: (title: string) => void;
  /** Shows a slide (by id) in the editor. */
  onShowSlide: (id: string) => void;
}

/** The FILE tab: the quiz as a whole and its file, over the whole window. */
export function Backstage(props: BackstageProps) {
  const { open, page, onPage, onClose, actions } = props;
  const { t } = useApp();
  const shown = open ? page : "open";
  const item = (action: Action) => (
    <button type="button" className="backstage-item" disabled={action.disabled} onClick={action.run}>
      {action.label}
    </button>
  );
  const pageItem = (id: BackstagePage, label: string) => (
    <button type="button" className={shown === id ? "backstage-item active" : "backstage-item"} disabled={id === "info" && !open} onClick={() => onPage(id)}>
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
        {item(actions.newQuiz)}
        {pageItem("open", t("openFile"))}
        {item(actions.save)}
        {item(actions.saveAs)}
        {item(actions.closeQuiz)}
        <span className="backstage-gap" />
        {item(actions.exit)}
      </nav>
      <div className="backstage-page">{shown === "info" && open ? <InfoPage {...props} open={open} /> : <OpenPage {...props} />}</div>
    </div>
  );
}

function InfoPage({ open, unsaved, onTitle, onShowSlide }: BackstageProps & { open: OpenQuiz }) {
  const { t } = useApp();
  const quiz = open.history.present;
  const slideNumber = new Map(quiz.slides.map((slide, i) => [slide.id, i + 1]));
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
        <ul className="problem-list">
          {open.problems.map((problem, i) => {
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
                <span>{problem.message}</span>
              </li>
            );
          })}
        </ul>
      )}
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
        <ul className="recent-list">
          {recentFiles.map((path) => (
            <li key={path}>
              <button type="button" onClick={() => onOpenFile(path)}>
                <b>{fileName(path)}</b>
                <small>{path}</small>
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

function fileName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1);
}
