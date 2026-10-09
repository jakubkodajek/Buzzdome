import type { MouseEvent } from "react";
import { useApp } from "../app-context";
import { previewView } from "../host/preview";
import { SlideScreen } from "../host/SlideScreen";
import { Stage } from "../host/Stage";
import { SoundIcon } from "../icons";
import { mediaFileName } from "../media";
import type { SlideContext } from "../quiz";
import { SlideBackground } from "../slide-media";
import type { Quiz, Slide } from "../types";
import { elementAt, type ElementId } from "./elements";

interface DesignAreaProps {
  quiz: Quiz;
  index: number;
  slide: Slide;
  context: SlideContext;
  /** The part of the slide that is selected; outlined. */
  element: ElementId;
  onSelect: (element: ElementId) => void;
  onEdit: (element: ElementId) => void;
  onMenu: (event: MouseEvent, element: ElementId) => void;
}

/**
 * The selected slide as the audience will see it. A click selects the part
 * under the pointer, a double click edits it.
 */
export function DesignArea({ quiz, index, slide, context, element, onSelect, onEdit, onMenu }: DesignAreaProps) {
  const { t } = useApp();
  return (
    <div
      className="design-area"
      data-selected={element}
      onClick={(e) => onSelect(elementAt(e.target))}
      onDoubleClick={(e) => onEdit(elementAt(e.target))}
      onContextMenu={(e) => {
        e.preventDefault();
        const target = elementAt(e.target);
        onSelect(target);
        onMenu(e, target);
      }}
    >
      <Stage className="design-stage">
        <SlideBackground slide={slide} mode="preview" />
        <SlideScreen view={previewView(slide, index, quiz.slides.length, quiz.title)} context={context} clock={null} mode="preview" />
        {slide.sound && (
          <div className="sound-badge" data-element="sound" title={t("mediaSound")}>
            <SoundIcon />
            <span>{mediaFileName(slide.sound.file)}</span>
          </div>
        )}
      </Stage>
    </div>
  );
}

/** The quizmaster's notes of the slide, under the design area. */
export function NotesBox({ notes, onChange }: { notes: string; onChange: (notes: string) => void }) {
  const { t } = useApp();
  return <textarea className="notes-box" aria-label={t("notes")} placeholder={t("notesPlaceholder")} value={notes} onChange={(e) => onChange(e.target.value)} />;
}
