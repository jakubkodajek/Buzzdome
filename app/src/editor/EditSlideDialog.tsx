import { useEffect, useRef, useState } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import { SLIDE_TYPE_LABEL } from "../quiz";
import type { QuestionSlide, Slide } from "../types";
import { Dialog } from "./controls";
import type { ElementId } from "./elements";
import { removeAnswer, setAnswerAt, setCorrect } from "./slides";

const FORM_ID = "edit-slide";

/**
 * The slide's texts in one form: the question and its answers with the
 * right one ticked, the title and text, or the explanation, and the notes. OK applies them
 * as one change; Cancel leaves the slide as it was.
 */
export function EditSlideDialog(props: { slide: Slide; focus: ElementId; onApply: (slide: Slide) => void; onClose: () => void }) {
  const { slide, focus, onApply, onClose } = props;
  const { t } = useApp();
  const [draft, setDraft] = useState(slide);
  const form = useRef<HTMLFormElement>(null);

  useEffect(() => {
    // Start where the quizmaster double-clicked (the Dialog focuses the first field otherwise).
    const field = form.current?.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[data-field="${focus}"]`);
    if (!field) return;
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }, [focus]);

  return (
    <Dialog
      title={t("editSlideTitle", { type: t(SLIDE_TYPE_LABEL[slide.type]) })}
      onClose={onClose}
      footer={
        <>
          <button type="submit" form={FORM_ID} className="primary">
            {t("ok")}
          </button>
          <button type="button" onClick={onClose}>
            {t("cancel")}
          </button>
        </>
      }
    >
      <form
        ref={form}
        id={FORM_ID}
        className="edit-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (draft !== slide) onApply(draft);
          onClose();
        }}
        onKeyDown={(e) => {
          // Ctrl+Enter confirms from a multi-line field too.
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            e.currentTarget.requestSubmit();
          }
        }}
      >
        {draft.type === "question" && <QuestionFields question={draft} onChange={setDraft} />}
        {(draft.type === "billboard" || draft.type === "endOfRound") && (
          <label className="field">
            <span>{t("slideTitleField")}</span>
            <input data-field="title" value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} />
          </label>
        )}
        {draft.type === "billboard" && (
          <label className="field">
            <span>{t("slideTextField")}</span>
            <textarea data-field="text" rows={4} value={draft.text} onChange={(e) => setDraft({ ...draft, text: e.target.value })} />
          </label>
        )}
        {draft.type === "explanation" && (
          <label className="field">
            <span>{t("explanationField")}</span>
            <textarea
              data-field="text"
              rows={6}
              placeholder={t("explanationPlaceholder")}
              value={draft.text}
              onChange={(e) => setDraft({ ...draft, text: e.target.value })}
            />
          </label>
        )}
        <label className="field">
          <span>{t("notes")}</span>
          <textarea rows={3} value={draft.notes} onChange={(e) => setDraft({ ...draft, notes: e.target.value })} />
        </label>
      </form>
    </Dialog>
  );
}

function QuestionFields({ question, onChange }: { question: QuestionSlide; onChange: (question: QuestionSlide) => void }) {
  const { t } = useApp();
  const count = question.answers.length;
  return (
    <>
      <label className="field">
        <span>{t("questionField")}</span>
        <textarea data-field="question" rows={3} value={question.text} onChange={(e) => onChange({ ...question, text: e.target.value })} />
      </label>
      <fieldset className="answer-rows">
        <legend>{t("answersField")}</legend>
        {ANSWER_LETTERS.map((letter, i) => {
          // Rows past the answers are greyed; the first of them takes a new answer.
          const used = i < count;
          return (
            <div key={letter} className={used ? "answer-row" : "answer-row unused"}>
              <span className={`answer-letter answer-${i}`}>{letter}</span>
              <label className="answer-correct">
                <input
                  type="checkbox"
                  checked={question.correct === i}
                  disabled={!used || (question.correct === i && question.mode === "everyone")}
                  onChange={(e) => onChange(setCorrect(question, e.target.checked ? i : null))}
                />
                {t("correctAnswer")}
              </label>
              <input
                data-field={`answer-${i}`}
                aria-label={t("elementAnswer", { letter })}
                placeholder={i === count ? t("addAnswer") : ""}
                disabled={i > count}
                value={question.answers[i] ?? ""}
                onChange={(e) => onChange(setAnswerAt(question, i, e.target.value))}
              />
              <button
                type="button"
                className="mini"
                title={t("removeAnswer")}
                aria-label={`${t("removeAnswer")} ${letter}`}
                disabled={!used || removeAnswer(question, i) === question}
                onClick={() => onChange(removeAnswer(question, i))}
              >
                ×
              </button>
            </div>
          );
        })}
      </fieldset>
    </>
  );
}
