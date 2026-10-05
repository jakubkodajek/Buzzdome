import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { StringKey, Translate } from "../i18n";
import { ANSWER_LETTERS, type GameView, type Quiz, type Slide } from "../types";

const TYPE_LABEL: Record<Slide["type"], StringKey> = {
  billboard: "typeBillboard",
  question: "typeQuestion",
  endOfRound: "typeEndOfRound",
};

function slideText(slide: Slide): string {
  return slide.type === "question" ? slide.text : slide.title;
}

/** Slide table with the current slide highlighted, and a preview pane. */
export function QuestionsTab(props: { t: Translate; view: GameView; quiz: Quiz | null; onError: (m: string) => void }) {
  const { t, view, quiz, onError } = props;
  const [selected, setSelected] = useState<number | null>(null);
  const current = view.phase.phase === "lobby" || view.phase.phase === "final" ? null : view.slideIndex;
  const shown = selected ?? current ?? 0;
  const currentRow = useRef<HTMLTableRowElement>(null);

  // Follow the show: when it moves on, select the new current slide.
  useEffect(() => {
    setSelected(null);
    currentRow.current?.scrollIntoView({ block: "nearest" });
  }, [current]);

  if (!quiz) return null;
  let questionNo = 0;
  const preview = quiz.slides[shown];

  return (
    <div className="questions-tab">
      <div className="toolbar">
        <button disabled={selected === null || selected === current} onClick={() => selected !== null && api.dispatch({ type: "goTo", slide: selected }).catch((e) => onError(String(e)))}>
          {t("goToSelected")}
        </button>
        {view.nextSlideTitle && <span className="muted">{t("nextUp", { title: view.nextSlideTitle })}</span>}
      </div>
      <div className="split">
        <div className="table-wrap">
          <table className="grid">
            <thead>
              <tr>
                <th>{t("colIndex")}</th>
                <th>{t("colQuestionNo")}</th>
                <th>{t("colType")}</th>
                <th>{t("colContent")}</th>
              </tr>
            </thead>
            <tbody>
              {quiz.slides.map((slide, i) => {
                if (slide.type === "question") questionNo += 1;
                const classes = [i === current ? "current" : "", i === selected ? "selected" : ""].join(" ");
                return (
                  <tr key={slide.id} ref={i === current ? currentRow : undefined} className={classes} onClick={() => setSelected(i)} onDoubleClick={() => api.dispatch({ type: "goTo", slide: i }).catch((e) => onError(String(e)))}>
                    <td className="num">{i + 1}</td>
                    <td className="num">{slide.type === "question" ? questionNo : ""}</td>
                    <td>{t(TYPE_LABEL[slide.type])}</td>
                    <td className="ellipsis">{slideText(slide)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <div className="preview">
          {preview && <SlidePreview t={t} slide={preview} counts={shown === current ? view.answerCounts : null} />}
        </div>
      </div>
    </div>
  );
}

function SlidePreview({ t, slide, counts }: { t: Translate; slide: Slide; counts: number[] | null }) {
  return (
    <>
      <div className="preview-card">
        <h2 className="preview-title">{slideText(slide)}</h2>
        {slide.type === "question" && (
          <>
            <p className="muted center">
              {slide.mode === "everyone" ? t("everyone") : t("fastestFinger")} · {slide.timeLimitS ? `${slide.timeLimitS} s` : t("untimed")} ·{" "}
              {t("points", { n: slide.pointsEnd !== null ? `${slide.points}→${slide.pointsEnd}` : slide.points })}
              {slide.penalty > 0 && ` · −${slide.penalty}`}
            </p>
            <div className="preview-answers">
              {slide.answers.map((answer, i) => (
                <div key={i} className={i === slide.correct ? "preview-answer correct" : "preview-answer"}>
                  <span className={`dot dot-${i}`}>{ANSWER_LETTERS[i]}</span>
                  <span className="answer-label">{answer}</span>
                  {counts && <span className="count">{counts[i]}</span>}
                </div>
              ))}
            </div>
          </>
        )}
        {slide.type === "billboard" && slide.text && <p className="center">{slide.text}</p>}
      </div>
      <div className="notes-box">
        <b>{t("notes")}</b>
        <p>{slide.notes || "—"}</p>
      </div>
    </>
  );
}
