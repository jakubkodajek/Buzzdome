import { useEffect, useRef, useState, type FocusEvent, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { SlideThumbnail } from "../host/SlideThumbnail";
import { AddIcon, ExclamationIcon } from "../icons";
import type { SlideContext } from "../quiz";
import type { Quiz } from "../types";
import { tooltip, type Action } from "./actions";

interface ThumbnailsProps {
  quiz: Quiz;
  /** Per slide, from `slideContexts`. */
  contexts: SlideContext[];
  /** The slide on the canvas. */
  current: number;
  /** The ids of the selected slides, the current one among them. */
  selected: ReadonlySet<string>;
  /** Slide ids with problems to fix. */
  problems: ReadonlySet<string>;
  /** Slide ids with warnings only (`Checked::warnings`), badged more mildly. */
  warnings: ReadonlySet<string>;
  /** A click on a slide, with Ctrl (Cmd) or Shift held or not. */
  onSelect: (index: number, keys: { ctrl: boolean; shift: boolean }) => void;
  onOpen: (index: number) => void;
  /** Right click on a slide (`index`) or on the empty list below them (null). */
  onMenu: (event: MouseEvent, index: number | null) => void;
  /** The slide with id `dragged` was dropped into the gap before slide `gap` (the end when it is the count). */
  onMove: (dragged: string, gap: number) => void;
  /** What the button under the slides does: adds a question. */
  add: Action;
}

/**
 * A list's current item (a slide, a category) in sight and its one Tab stop
 * (a roving tab stop); while the list has the focus, the focus goes with it
 * as the arrow keys move it, so the menu key and Enter act on it. An item
 * deleted with the focus leaves it on the page, so the current item after
 * it takes the focus then too.
 */
export function useCurrentItem(current: number) {
  const currentItem = useRef<HTMLLIElement>(null);
  // The current item as last shown, to act once each time it changes.
  const shown = useRef<{ element: HTMLLIElement; current: number } | null>(null);
  // The item that had the focus last, until the focus went to another control.
  const focusedItem = useRef<HTMLLIElement | null>(null);
  useEffect(() => {
    const element = currentItem.current;
    if (!element || (shown.current?.element === element && shown.current.current === current)) return;
    shown.current = { element, current };
    element.scrollIntoView({ block: "nearest" });
    const focused = document.activeElement;
    const inList = focused instanceof HTMLLIElement && element.parentElement?.contains(focused);
    const lostWithItem = focused === document.body && focusedItem.current !== null && !focusedItem.current.isConnected;
    if (focused !== element && (inList || lostWithItem)) element.focus();
  });
  return (i: number) => ({
    ref: i === current ? currentItem : undefined,
    tabIndex: i === current ? 0 : -1,
    onFocus: (e: FocusEvent<HTMLLIElement>) => (focusedItem.current = e.currentTarget),
    onBlur: (e: FocusEvent<HTMLLIElement>) => {
      if (e.relatedTarget) focusedItem.current = null;
    },
  });
}

/** The slides down the left side: click to select (Ctrl or Shift for several), drag to reorder. */
export function Thumbnails({ quiz, contexts, current, selected, problems, warnings, onSelect, onOpen, onMenu, onMove, add }: ThumbnailsProps) {
  const { t } = useApp();
  // While dragging: the slide dragged and the gap (0..count) it would drop into.
  const [drag, setDrag] = useState<{ from: number; gap: number } | null>(null);
  const item = useCurrentItem(current);
  // Gaps next to a slide dragged alone would leave it where it is: no marker.
  const alone = drag !== null && !(selected.size > 1 && selected.has(quiz.slides[drag.from]?.id ?? ""));
  const gap = drag && !(alone && (drag.gap === drag.from || drag.gap === drag.from + 1)) ? drag.gap : null;

  const drop = () => {
    if (!drag) return;
    setDrag(null);
    const dragged = quiz.slides[drag.from];
    if (dragged) onMove(dragged.id, drag.gap);
  };

  return (
    <nav
      className="thumbnails"
      aria-label={t("slides")}
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu(e, null);
      }}
    >
      <ol>
        {quiz.slides.map((slide, i) => (
          <li
            key={slide.id}
            {...item(i)}
            aria-selected={selected.has(slide.id)}
            className={[
              "thumbnail",
              i === current ? "selected" : selected.has(slide.id) ? "selected also" : "",
              gap === i ? "drop-before" : "",
              gap === i + 1 && i === quiz.slides.length - 1 ? "drop-after" : "",
            ].join(" ")}
            draggable
            onClick={(e) => onSelect(i, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })}
            onDoubleClick={() => onOpen(i)}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              // The menu acts on the selection, so a slide outside it is selected alone first.
              if (!selected.has(slide.id)) onSelect(i, { ctrl: false, shift: false });
              onMenu(e, i);
            }}
            onDragStart={(e) => {
              e.dataTransfer.effectAllowed = "move";
              e.dataTransfer.setData("text/plain", slide.id);
              setDrag({ from: i, gap: i });
            }}
            onDragOver={(e) => {
              if (!drag) return;
              e.preventDefault();
              const rect = e.currentTarget.getBoundingClientRect();
              const gap = e.clientY > rect.top + rect.height / 2 ? i + 1 : i;
              setDrag((current) => (current && current.gap !== gap ? { ...current, gap } : current));
            }}
            onDrop={(e) => {
              e.preventDefault();
              drop();
            }}
            onDragEnd={() => setDrag(null)}
          >
            <span className="thumbnail-number">{i + 1}</span>
            <div className="thumbnail-frame">
              <SlideThumbnail
                className="thumbnail-stage"
                slide={slide}
                index={i}
                count={quiz.slides.length}
                context={contexts[i]}
              />
              {problems.has(slide.id) ? (
                <span className="thumbnail-problem" title={t("slideHasProblems")}>
                  <ExclamationIcon />
                </span>
              ) : (
                warnings.has(slide.id) && (
                  <span className="thumbnail-problem warning" title={t("slideHasWarnings")}>
                    <ExclamationIcon />
                  </span>
                )
              )}
            </div>
          </li>
        ))}
      </ol>
      <button type="button" className="add-slide" title={tooltip(add)} aria-label={add.label} disabled={add.disabled} onClick={add.run}>
        <AddIcon />
      </button>
    </nav>
  );
}
