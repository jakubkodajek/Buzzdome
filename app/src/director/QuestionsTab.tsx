import { useEffect, useRef, useState } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp, useDispatch } from "../app-context";
import type { StringKey } from "../i18n";
import { questionNumbers, slideTitle } from "../quiz";
import { SlideMedia } from "../slide-media";
import type { GameView, Quiz, Slide } from "../types";

const TYPE_LABEL: Record<Slide["type"], StringKey> = {
  billboard: "typeBillboard",
  question: "typeQuestion",
  endOfRound: "typeEndOfRound",
};

/** Slide table with the current slide highlighted, and a preview pane. */
export function QuestionsTab({ view, quiz }: { view: GameView; quiz: Quiz | null }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const current = view.phase.phase === "lobby" || view.phase.phase === "final" ? null : view.slideIndex;
  // A selection lasts until the show moves on; then the current slide is shown again.
  const [selection, setSelection] = useState<{ slide: number; madeOn: number | null } | null>(null);
  const selected = selection !== null && selection.madeOn === current ? selection.slide : null;
  const setSelected = (slide: number) => setSelection({ slide, madeOn: current });
  const shown = selected ?? current ?? 0;
  const currentRow = useRef<HTMLTableRowElement>(null);

  useEffect(() => {
    currentRow.current?.scrollIntoView({ block: "nearest" });
  }, [current]);

  if (!quiz) return null;
  const numbers = questionNumbers(quiz);
  const preview = quiz.slides[shown];
  const goTo = (slide: number) => dispatch({ type: "goTo", slide });

  return (
    <div className="questions-tab">
      <div className="toolbar">
        <button disabled={selected === null || selected === current} onClick={() => selected !== null && goTo(selected)}>
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
              {quiz.slides.map((slide, i) => (
                <tr
                  key={slide.id}
                  ref={i === current ? currentRow : undefined}
                  className={[i === current ? "current" : "", i === selected ? "selected" : ""].join(" ")}
                  onClick={() => setSelected(i)}
                  onDoubleClick={() => goTo(i)}
                >
                  <td className="num">{i + 1}</td>
                  <td className="num">{numbers[i] ?? ""}</td>
                  <td>{t(TYPE_LABEL[slide.type])}</td>
                  <td className="ellipsis">{slideTitle(slide)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="preview">{preview && <SlidePreview slide={preview} counts={shown === current ? view.answerCounts : null} />}</div>
      </div>
    </div>
  );
}

function SlidePreview({ slide, counts }: { slide: Slide; counts: number[] | null }) {
  const { t } = useApp();
  return (
    <>
      <div className="preview-card">
        <h2 className="preview-title">{slideTitle(slide)}</h2>
        <SlideMedia slide={slide} className="preview-media" preview />
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
      <MediaList slide={slide} />
      <div className="notes-box">
        <b>{t("notes")}</b>
        <p>{slide.notes || "—"}</p>
      </div>
    </>
  );
}

/** The slide's media files by name, so the quizmaster knows what will play. */
function MediaList({ slide }: { slide: Slide }) {
  const { t } = useApp();
  const items: [StringKey, string | null | undefined][] = [
    ["mediaPicture", slide.picture],
    ["mediaVideo", slide.video?.file],
    ["mediaSound", slide.sound?.file],
    ["mediaBackground", slide.background],
  ];
  const present = items.filter((item): item is [StringKey, string] => Boolean(item[1]));
  if (present.length === 0) return null;
  return (
    <dl className="media-list">
      {present.map(([label, file]) => (
        <div key={label}>
          <dt>{t(label)}</dt>
          <dd>{file.slice("media/".length)}</dd>
        </div>
      ))}
    </dl>
  );
}
