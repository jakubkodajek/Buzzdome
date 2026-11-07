import { useState, type FormEvent } from "react";
import { useApp } from "../app-context";
import { EditIcon, FolderIcon, AddIcon } from "../icons";
import { quizName } from "../quiz";
import { RecentFileList } from "../recent-files";
import { Dialog } from "./controls";
import type { OpenQuiz } from "./editor-state";

/** What the quizmaster can start with: a new quiz, named; a quiz file; or the quiz already open. */
interface StartChoices {
  /** Creates a new quiz called `title` (the untitled name when empty). */
  onNew: (title: string) => void;
  /** Opens `recent`, a file in `recentFiles`, or one the quizmaster picks. */
  onOpen: (recent?: string) => void;
  onForget: (recent: string) => void;
  recentFiles: string[];
  /** A file operation is under way: nothing else starts. */
  busy: boolean;
}

/**
 * Asked as the editor opens from the Director: carry on with the quiz left
 * open, start a new one under a name, or open a quiz file. Closing it goes
 * back (`onCancel`).
 */
export function QuizStartDialog(props: StartChoices & { open: OpenQuiz | null; unsaved: boolean; onContinue: () => void; onCancel: () => void }) {
  const { open, unsaved, onContinue, onCancel, onOpen, onForget, recentFiles, busy } = props;
  const { t } = useApp();
  const title = open ? quizName(t, open.history.present) : null;
  return (
    <Dialog
      title={t("quizEditor")}
      className="quiz-start-dialog"
      staysOpen
      onClose={onCancel}
      footer={
        <button type="button" disabled={busy} onClick={onCancel}>
          {t("cancel")}
        </button>
      }
    >
      {open && (
        <section>
          <h3>{t("continueEditing")}</h3>
          <button type="button" className="start-choice continue" data-autofocus disabled={busy} onClick={onContinue}>
            <EditIcon />
            <span>
              <b>{title}</b>
              <small>
                {open.sourcePath ?? t("notSavedYet")}
                {unsaved && ` · ${t("unsaved")}`}
              </small>
            </span>
          </button>
        </section>
      )}
      <section>
        <h3>{t("newQuizHeading")}</h3>
        <NewQuizForm onNew={props.onNew} busy={busy} autoFocus={!open} />
      </section>
      <section>
        <h3>{t("openExistingQuiz")}</h3>
        <button type="button" className="start-choice" disabled={busy} onClick={() => onOpen()}>
          <FolderIcon />
          <span>
            <b>{t("openQuiz")}</b>
            <small>{t("openQuizHint")}</small>
          </span>
        </button>
        {recentFiles.length > 0 && (
          <>
            <h4>{t("recentFiles")}</h4>
            <RecentFileList files={recentFiles} disabled={busy} onOpen={onOpen} onForget={onForget} />
          </>
        )}
      </section>
    </Dialog>
  );
}

/** Asks for the name of a new quiz (File › New, Ctrl+N). */
export function NewQuizDialog({ onNew, onCancel, busy }: { onNew: (title: string) => void; onCancel: () => void; busy: boolean }) {
  const { t } = useApp();
  return (
    <Dialog
      title={t("newQuizHeading")}
      className="quiz-start-dialog"
      staysOpen
      onClose={onCancel}
      footer={
        <button type="button" disabled={busy} onClick={onCancel}>
          {t("cancel")}
        </button>
      }
    >
      <NewQuizForm onNew={onNew} busy={busy} autoFocus />
    </Dialog>
  );
}

/** The new quiz's name and the button that creates it; Enter creates it too. */
function NewQuizForm({ onNew, busy, autoFocus }: { onNew: (title: string) => void; busy: boolean; autoFocus: boolean }) {
  const { t } = useApp();
  const [title, setTitle] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!busy) onNew(title.trim());
  };
  return (
    <form className="new-quiz-form" onSubmit={submit}>
      <label className="field">
        <span>{t("quizTitle")}</span>
        <input value={title} placeholder={t("untitledQuiz")} data-autofocus={autoFocus || undefined} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <button type="submit" className="primary" disabled={busy}>
        <AddIcon />
        {t("createQuiz")}
      </button>
    </form>
  );
}
