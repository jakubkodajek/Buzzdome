import { useEffect, useRef, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { SlideThumbnail } from "../host/SlideThumbnail";
import type { SlideContext } from "../quiz";
import type { Quiz } from "../types";

interface ThumbnailsProps {
  quiz: Quiz;
  /** Per slide, from `slideContexts`. */
  contexts: SlideContext[];
  selected: number;
  /** Slide ids with problems. */
  problems: ReadonlySet<string>;
  onSelect: (index: number) => void;
  onOpen: (index: number) => void;
  /** Right click on a slide (`index`) or on the empty list below them (null). */
  onMenu: (event: MouseEvent, index: number | null) => void;
  onMove: (from: number, to: number) => void;
  onAdd: () => void;
}

/** The slides down the left side: click to select, drag to reorder. */
export function Thumbnails({ quiz, contexts, selected, problems, onSelect, onOpen, onMenu, onMove, onAdd }: ThumbnailsProps) {
  const { t } = useApp();
  // While dragging: the slide dragged and the gap (0..count) it would drop into.
  const [drag, setDrag] = useState<{ from: number; gap: number } | null>(null);
  const selectedItem = useRef<HTMLLIElement>(null);
  // Gaps next to the dragged slide would leave it where it is: no marker.
  const gap = drag && drag.gap !== drag.from && drag.gap !== drag.from + 1 ? drag.gap : null;

  useEffect(() => {
    selectedItem.current?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const drop = () => {
    if (!drag) return;
    const to = drag.gap > drag.from ? drag.gap - 1 : drag.gap;
    setDrag(null);
    if (to !== drag.from) onMove(drag.from, to);
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
            ref={i === selected ? selectedItem : undefined}
            className={[
              "thumbnail",
              i === selected ? "selected" : "",
              gap === i ? "drop-before" : "",
              gap === i + 1 && i === quiz.slides.length - 1 ? "drop-after" : "",
            ].join(" ")}
            draggable
            onClick={() => onSelect(i)}
            onDoubleClick={() => onOpen(i)}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onSelect(i);
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
