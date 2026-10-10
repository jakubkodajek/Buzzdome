import { useEffect, useRef, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { SlideThumbnail } from "../host/SlideThumbnail";
import type { SlideContext } from "../quiz";
import type { Quiz } from "../types";

interface ThumbnailsProps {
  quiz: Quiz;
  /** Per slide, from `slideContexts`. */
  contexts: SlideContext[];
  /** The slide on the canvas. */
  current: number;
  /** The ids of the selected slides, the current one among them. */
  selected: ReadonlySet<string>;
  /** Slide ids with problems. */
  problems: ReadonlySet<string>;
  /** A click on a slide, with Ctrl (Cmd) or Shift held or not. */
  onSelect: (index: number, keys: { ctrl: boolean; shift: boolean }) => void;
  onOpen: (index: number) => void;
  /** Right click on a slide (`index`) or on the empty list below them (null). */
  onMenu: (event: MouseEvent, index: number | null) => void;
  /** The slide with id `dragged` was dropped into the gap before slide `gap` (the end when it is the count). */
  onMove: (dragged: string, gap: number) => void;
  onAdd: () => void;
}

/** The slides down the left side: click to select (Ctrl or Shift for several), drag to reorder. */
export function Thumbnails({ quiz, contexts, current, selected, problems, onSelect, onOpen, onMenu, onMove, onAdd }: ThumbnailsProps) {
  const { t } = useApp();
  // While dragging: the slide dragged and the gap (0..count) it would drop into.
  const [drag, setDrag] = useState<{ from: number; gap: number } | null>(null);
  const selectedItem = useRef<HTMLLIElement>(null);
  // Gaps next to a slide dragged alone would leave it where it is: no marker.
  const alone = drag !== null && !(selected.size > 1 && selected.has(quiz.slides[drag.from]?.id ?? ""));
  const gap = drag && !(alone && (drag.gap === drag.from || drag.gap === drag.from + 1)) ? drag.gap : null;

  useEffect(() => {
    selectedItem.current?.scrollIntoView({ block: "nearest" });
  }, [current]);

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
            ref={i === current ? selectedItem : undefined}
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
              {problems.has(slide.id) && (
                <span className="thumbnail-problem" title={t("slideHasProblems")}>
                  !
                </span>
              )}
            </div>
          </li>
        ))}
      </ol>
      <button type="button" className="add-slide" title={t("newSlide")} aria-label={t("newSlide")} onClick={onAdd}>
        +
      </button>
    </nav>
  );
}
