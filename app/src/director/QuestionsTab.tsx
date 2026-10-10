import { useEffect, useRef, useState, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { FULL_SHARE } from "../bindings/Constants";
import { useApp, useDispatch, useRun } from "../app-context";
import { SlideBoundary } from "../host/SlideBoundary";
import { SlideThumbnail } from "../host/SlideThumbnail";
import type { StringKey } from "../i18n";
import { NextIcon, TrophyIcon } from "../icons";
import { mediaFileName } from "../media";
import { readPreference, writePreference } from "../preferences";
import { shareOf, SLIDE_TYPE_LABEL, slideContexts, slideTitle, type SlideContext } from "../quiz";
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
  // None in the lobby and in the ranking after the last slide.
  const current = view.phase.phase === "lobby" || view.slideIndex >= view.slideCount ? null : view.slideIndex;
  // A selection lasts until the show moves on; then the current slide is shown again.
  const [selection, setSelection] = useState<{ slide: number; madeOn: number | null } | null>(null);
  const selected = selection !== null && selection.madeOn === current ? selection.slide : null;
  const setSelected = (slide: number) => setSelection({ slide, madeOn: current });
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
  // Jumps only with the buttons, never mid-question, and the final asks first (A3).
  const jumps = view.controls.goTo;
  const goToFinal = () => run(confirmAction(t, t("goToFinalConfirm")).then((yes) => yes && dispatch({ type: "goToFinal" })));

  return (
    <div className="questions-tab">
      <div className="toolbar dark-toolbar">
        <ToolButton
          icon={<NextIcon />}
          disabled={!jumps || selected === null || selected === current}
          onClick={() => selected !== null && dispatch({ type: "goTo", slide: selected })}
        >
          {t("goToSelected")}
        </ToolButton>
        <ToolButton icon={<TrophyIcon />} disabled={!jumps || view.phase.phase === "final"} onClick={goToFinal}>
          {t("goToFinal")}
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
                  onKeyDown={(e) => {
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

function ToolButton(props: { icon: ReactNode; pressed?: boolean; disabled?: boolean; onClick: () => void; children: ReactNode }) {
  const { icon, pressed, disabled, onClick, children } = props;
  return (
    <button type="button" className="tool-button" aria-pressed={pressed} disabled={disabled} onClick={onClick}>
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
                    {share !== null && share < FULL_SHARE && <span className="share">{share} %</span>}
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
            {explained && explained.question.correct.length > 0 && (
              <div className="preview-answers">
                {explained.question.correct.map(({ answer }) => (
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
