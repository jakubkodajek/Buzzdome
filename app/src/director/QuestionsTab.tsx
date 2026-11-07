import { useEffect, useRef, useState, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { FULL_SHARE } from "../bindings/Constants";
import { useApp, useDispatch, useRun } from "../app-context";
import { SlideBoundary } from "../host/SlideBoundary";
import { SlideThumbnail } from "../host/SlideThumbnail";
import type { StringKey } from "../i18n";
import { NextIcon, TrophyIcon, UndoIcon } from "../icons";
import { mediaFileName } from "../media";
import { beforeSlides, currentSlide } from "../phase";
import { readPreference, writePreference } from "../preferences";
import { rightAnswers, shareOf, SLIDE_TYPE_LABEL, slideContexts, slideTitle, type SlideContext } from "../quiz";
import { SlideMedia } from "../slide-media";
import type { GameView, Quiz, Slide } from "../types";
import { confirmAction } from "../confirm";

type Layout = "horizontal" | "vertical";

const LAYOUT_KEY = "quiz-show.questions-layout";

function readLayout(): Layout {
  return readPreference(LAYOUT_KEY) === "vertical" ? "vertical" : "horizontal";
}

/** Slide table with the current slide highlighted, and a preview pane beside or under it. */
export function QuestionsTab({ view, quiz }: { view: GameView; quiz: Quiz | null }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const current = currentSlide(view);
  // A selection lasts until the show moves on or the quizmaster goes back
  // (Esc, or the button over the preview); then the current slide is shown again.
  const [selection, setSelection] = useState<{ slide: number; madeOn: number | null } | null>(null);
  const selected = selection !== null && selection.madeOn === current ? selection.slide : null;
  const setSelected = (slide: number) => setSelection({ slide, madeOn: current });
  const looksAhead = selected !== null && selected !== current;
  const shown = selected ?? current ?? 0;
  const currentRow = useRef<HTMLTableRowElement>(null);
  const [layout, setLayoutState] = useState<Layout>(readLayout);
  const setLayout = (next: Layout) => {
    setLayoutState(next);
    writePreference(LAYOUT_KEY, next);
  };

  useEffect(() => {
    currentRow.current?.scrollIntoView({ block: "nearest" });
  }, [current]);

  if (!quiz) return null;
  const contexts = slideContexts(quiz);
  const preview = quiz.slides[shown];
  const nextSlide = view.nextSlide === null ? undefined : quiz.slides[view.nextSlide];
  // Jumps never mid-question (the buttons say why); the final and starting over ask first.
  const jumps = view.controls.goTo;
  const noJumps = jumps ? undefined : t("jumpsNotMidQuestion");
  const goTo = (slide: number) => jumps && slide !== current && dispatch({ type: "goTo", slide });
  const goToFinal = () => run(confirmAction(t, t("goToFinalConfirm"), t("goToFinal")).then((yes) => yes && dispatch({ type: "goToFinal" })));
  const startOver = () => run(confirmAction(t, t("startOverConfirm"), t("startOver")).then((yes) => yes && dispatch({ type: "startOver" })));

  return (
    <div className="questions-tab">
      <div className="toolbar dark-toolbar">
        <ToolButton icon={<NextIcon />} title={noJumps} disabled={!jumps || !looksAhead} onClick={() => selected !== null && goTo(selected)}>
          {t("goToSelected")}
        </ToolButton>
        <ToolButton icon={<TrophyIcon />} title={noJumps} disabled={!jumps || view.phase.phase === "final"} onClick={goToFinal}>
          {t("goToFinal")}
        </ToolButton>
        <ToolButton icon={<UndoIcon />} title={noJumps} disabled={!jumps || beforeSlides(view.phase)} onClick={startOver}>
          {t("startOver")}
        </ToolButton>
        <span className="toolbar-separator" />
        <span>{t("layout")}</span>
        <ToolButton icon={<LayoutIcon vertical={false} />} pressed={layout === "horizontal"} onClick={() => setLayout("horizontal")}>
          {t("layoutHorizontal")}
        </ToolButton>
        <ToolButton icon={<LayoutIcon vertical />} pressed={layout === "vertical"} onClick={() => setLayout("vertical")}>
          {t("layoutVertical")}
        </ToolButton>
        <span className="spacer" />
        {nextSlide && <span className="muted">{t("nextUp", { title: slideTitle(nextSlide) })}</span>}
      </div>
      <div className={`split ${layout}`}>
        <div className="table-wrap">
          <table className="grid slide-grid">
            <thead>
              <tr>
                <th>{t("colIndex")}</th>
                <th>{t("colQuestionNo")}</th>
                <th>{t("colThumbnail")}</th>
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
                  tabIndex={0}
                  aria-selected={i === shown}
                  onClick={() => setSelected(i)}
                  onFocus={() => setSelected(i)}
                  onDoubleClick={() => goTo(i)}
                  onKeyDown={(e) => {
                    // Enter jumps to the row, Esc goes back to the current slide.
                    if (e.key === "Enter" || e.key === "Escape") {
                      e.preventDefault();
                      if (e.key === "Enter") goTo(i);
                      else setSelection(null);
                      return;
                    }
                    // The arrows move along the rows; Space stays the show's Next.
                    const row = { ArrowDown: e.currentTarget.nextElementSibling, ArrowUp: e.currentTarget.previousElementSibling }[e.key];
                    if (!(row instanceof HTMLElement)) return;
                    e.preventDefault();
                    row.focus();
                  }}
                >
                  <td className="num">{i + 1}</td>
                  <td className="num">{contexts[i].position?.n ?? ""}</td>
                  <td className="thumb-cell">
                    <SlideThumbnail
                      className="grid-thumbnail"
                      slide={slide}
                      index={i}
                      count={quiz.slides.length}
                      context={contexts[i]}
                    />
                  </td>
                  <td>{t(SLIDE_TYPE_LABEL[slide.type])}</td>
                  <td className="ellipsis">{slideTitle(slide)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="preview">
          {looksAhead && (
            <div className="preview-ahead" role="status">
              <span>{current === null ? t("previewSelectedNoSlide", { n: selected + 1 }) : t("previewSelected", { n: selected + 1, m: current + 1 })}</span>
              <button type="button" onClick={() => setSelection(null)}>
                {t("backToCurrent")}
              </button>
            </div>
          )}
          {preview && (
            <SlideBoundary className="preview-card" slide={preview}>
              <SlidePreview slide={preview} context={contexts[shown]} counts={shown === current ? view.answerCounts : null} />
            </SlideBoundary>
          )}
        </div>
      </div>
    </div>
  );
}

function ToolButton(props: { icon: ReactNode; title?: string; pressed?: boolean; disabled?: boolean; onClick: () => void; children: ReactNode }) {
  const { icon, title, pressed, disabled, onClick, children } = props;
  return (
    <button type="button" className="tool-button" title={title} aria-pressed={pressed} disabled={disabled} onClick={onClick}>
      {icon}
      {children}
    </button>
  );
}

/** Table and preview side by side, or one above the other. */
function LayoutIcon({ vertical }: { vertical: boolean }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
      <rect x="3" y="4" width="18" height="16" rx="1" />
      {vertical ? <path d="M3 12h18" /> : <path d="M12 4v16" />}
    </svg>
  );
}

function SlidePreview({ slide, context, counts }: { slide: Slide; context: SlideContext; counts: number[] | null }) {
  const { t } = useApp();
  const explained = context.explains;
  const explainedRight = explained ? rightAnswers(explained.question) : [];
  return (
    <>
      <div className="preview-card">
        <h2 className="preview-title">{slide.type === "explanation" ? t("typeExplanation") : slideTitle(slide)}</h2>
        <SlideMedia slide={slide} className="preview-media" mode="preview" />
        {slide.type === "question" && (
          <>
            <p className="muted center">
              {[
                t(slide.mode === "everyone" ? "everyone" : "fastestFinger"),
                slide.timeLimitS ? t("timeLimitSeconds", { n: slide.timeLimitS }) : t("untimed"),
                t("points", { n: slide.pointsEnd !== null ? `${slide.points}→${slide.pointsEnd}` : slide.points }),
                slide.penalty > 0 && t("penaltyPoints", { n: slide.penalty }),
              ]
                .filter(Boolean)
                .join(" · ")}
            </p>
            <div className="preview-answers">
              {slide.answers.map((answer, i) => {
                const share = shareOf(slide, i);
                return (
                  <div key={i} className={share !== null ? "preview-answer correct" : "preview-answer"}>
                    <span className={`dot dot-${i}`}>{ANSWER_LETTERS[i]}</span>
                    <span className="answer-label">{answer}</span>
                    {share !== null && share < FULL_SHARE && <span className="share">{t("percent", { n: share })}</span>}
                    {counts && <span className="count">{counts[i]}</span>}
                  </div>
                );
              })}
            </div>
          </>
        )}
        {slide.type === "billboard" && slide.text && <p className="center pre-line">{slide.text}</p>}
        {slide.type === "explanation" && (
          <>
            <p className="muted center">
              {explained ? `${t("questionNumber", { n: explained.position.n })}: ${explained.question.text}` : t("noQuestionToExplain")}
            </p>
            {explained && explainedRight.length > 0 && (
              <div className="preview-answers">
                {explainedRight.map(({ answer }) => (
                  <div key={answer} className="preview-answer correct">
                    <span className={`dot dot-${answer}`}>{ANSWER_LETTERS[answer]}</span>
                    <span className="answer-label">{explained.question.answers[answer]}</span>
                  </div>
                ))}
              </div>
            )}
            <p className="center pre-line">{slide.text}</p>
          </>
        )}
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
    ["mediaBackground", slide.background?.file],
  ];
  const present = items.filter((item): item is [StringKey, string] => Boolean(item[1]));
  if (present.length === 0) return null;
  return (
    <dl className="media-list">
      {present.map(([label, file]) => (
        <div key={label}>
          <dt>{t(label)}</dt>
          <dd>{mediaFileName(file)}</dd>
        </div>
      ))}
    </dl>
  );
}
