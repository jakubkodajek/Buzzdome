import { useState, type MouseEvent, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import type { StringKey } from "../i18n";
import { isCorrect, slideTitle } from "../quiz";
import type { Quiz, Slide } from "../types";
import type { Action } from "./actions";
import { ContextMenu } from "./controls";
import { slideProperties, type SlideEditing } from "./properties";
import { addAnswer, setAnswerAt, setHeadline, setNotes } from "./slides";

type ColumnId = "answers" | "correct" | "time" | "points" | "pointsEnd" | "penalty" | "notes";

const COLUMNS: [ColumnId, StringKey][] = [
  ["answers", "answersField"],
  ["correct", "propCorrectAnswer"],
  ["time", "propAnswerTime"],
  ["points", "propPointsStart"],
  ["pointsEnd", "propPointsEnd"],
  ["penalty", "propPenalty"],
  ["notes", "notes"],
];

interface TableViewProps {
  quiz: Quiz;
  selected: number;
  onSelect: (index: number) => void;
  onOpen: (index: number) => void;
  /** How a row's slide is changed. */
  editingFor: (slide: Slide) => SlideEditing;
}

/**
 * The quiz as a grid, a row per slide, for typing many questions quickly.
 * Right-click the header to choose the columns.
 */
export function TableView({ quiz, selected, onSelect, onOpen, editingFor }: TableViewProps) {
  const { t } = useApp();
  const [hidden, setHidden] = useState<ReadonlySet<ColumnId>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const shown = (column: ColumnId) => !hidden.has(column);

  const columnActions: Action[] = COLUMNS.map(([column, label]) => ({
    label: t(label),
    checked: shown(column),
    run: () =>
      setHidden((current) => {
        const next = new Set(current);
        if (!next.delete(column)) next.add(column);
        return next;
      }),
  }));

  const openMenu = (e: MouseEvent) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY });
  };

  return (
    <div className="table-view">
      <table className="slide-table">
        <thead onContextMenu={openMenu} title={t("columnsHint")}>
          <tr>
            <th>#</th>
            <th>{t("colType")}</th>
            <th className="wide">{t("colContent")}</th>
            {shown("answers") && ANSWER_LETTERS.map((letter) => <th key={letter}>{t("elementAnswer", { letter })}</th>)}
            {COLUMNS.slice(1).map(([column, label]) => shown(column) && <th key={column}>{t(label)}</th>)}
          </tr>
        </thead>
        <tbody>
          {quiz.slides.map((slide, i) => (
            <SlideRow
              key={slide.id}
              index={i}
              selected={i === selected}
              editing={editingFor(slide)}
              shown={shown}
              onSelect={() => onSelect(i)}
              onOpen={() => onOpen(i)}
            />
          ))}
        </tbody>
      </table>
      {quiz.slides.length === 0 && <p className="table-empty">{t("noSlides")}</p>}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={columnActions} onClose={() => setMenu(null)} />}
    </div>
  );
}

function SlideRow(props: { index: number; selected: boolean; editing: SlideEditing; shown: (column: ColumnId) => boolean; onSelect: () => void; onOpen: () => void }) {
  const { index, selected, editing, shown, onSelect, onOpen } = props;
  const { t } = useApp();
  const { slide, update } = editing;
  const rows = slideProperties(t, editing);
  const q = slide.type === "question" ? slide : null;
  const cell = (column: ColumnId, content: ReactNode) => shown(column) && <td className={q ? "" : "na"}>{content}</td>;

  return (
    <tr className={selected ? "selected" : ""} onFocusCapture={onSelect} onClick={onSelect}>
      <th scope="row" className="row-number" onDoubleClick={onOpen}>
        {index + 1}
      </th>
      <td>{rows.type.control}</td>
      <td className="wide">
        <input
          aria-label={t("colContent")}
          value={slideTitle(slide)}
          onChange={(e) => {
            const text = e.target.value;
            update((s) => setHeadline(s, text), "text");
          }}
        />
      </td>
      {shown("answers") &&
        ANSWER_LETTERS.map((letter, i) => {
          // The answers, then a cell to type a new one into if the mode allows one.
          const editable = q !== null && (i < q.answers.length || (i === q.answers.length && addAnswer(q) !== q));
          return (
            <td key={letter} className={editable ? (isCorrect(q, i) ? "correct" : "") : "na"}>
              {editable && (
                <input
                  aria-label={t("elementAnswer", { letter })}
                  value={q.answers[i] ?? ""}
                  onChange={(e) => {
                    const text = e.target.value;
                    update((s) => (s.type === "question" ? setAnswerAt(s, i, text) : s), `answer${i}`);
                  }}
                />
              )}
            </td>
          );
        })}
      {cell("correct", q && rows.correctAnswer(q).control)}
      {cell("time", q && rows.answerTime(q).control)}
      {cell("points", q && rows.pointsStart(q).control)}
      {cell(
        "pointsEnd",
        q && (
          <span className="falling-cell">
            {rows.falling(q).control}
            {q.pointsEnd !== null && rows.pointsEnd(q).control}
          </span>
        ),
      )}
      {cell("penalty", q && rows.penalty(q).control)}
      {shown("notes") && (
        <td>
          <input
            aria-label={t("notes")}
            value={slide.notes}
            onChange={(e) => {
              const notes = e.target.value;
              update((s) => setNotes(s, notes), "notes");
            }}
          />
        </td>
      )}
    </tr>
  );
}
