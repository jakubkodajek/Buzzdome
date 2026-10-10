import type { MouseEvent } from "react";
import { useApp } from "../app-context";
import { previewView } from "../host/preview";
import { SlideBoundary } from "../host/SlideBoundary";
import { SlideScreen } from "../host/SlideScreen";
import { Stage } from "../host/Stage";
import { SoundIcon } from "../icons";
import { mediaFileName } from "../media";
import type { SlideContext } from "../quiz";
import { screenOf } from "../theme/themes";
import type { Quiz, Slide } from "../types";
import { elementAt, type ElementId } from "./elements";

interface DesignAreaProps {
  quiz: Quiz;
  index: number;
  slide: Slide;
  context: SlideContext;
  /** The part of the slide that is selected; outlined. */
  element: ElementId;
  /** The background video plays (it shows its first frame otherwise). */
  playingBackground: boolean;
  onSelect: (element: ElementId) => void;
  onEdit: (element: ElementId) => void;
  onMenu: (event: MouseEvent, element: ElementId) => void;
}

/**
 * The selected slide as the audience will see it. A click selects the part
 * under the pointer, a double click edits it.
 */
export function DesignArea({ quiz, index, slide, context, element, playingBackground, onSelect, onEdit, onMenu }: DesignAreaProps) {
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
      <SlideBoundary className="design-stage" slide={slide}>
        <DesignStage quiz={quiz} index={index} slide={slide} context={context} playingBackground={playingBackground} />
      </SlideBoundary>
    </div>
  );
}

/** The slide drawn in the design area, with its sound's name. */
function DesignStage({ quiz, index, slide, context, playingBackground }: Pick<DesignAreaProps, "quiz" | "index" | "slide" | "context" | "playingBackground">) {
  const { t } = useApp();
  const view = previewView(slide, index, quiz.slides.length, quiz.title);
  return (
    <Stage className="design-stage" screen={screenOf(view)} slide={slide} mode="preview" playingBackground={playingBackground}>
      <SlideScreen view={view} context={context} clock={null} mode="preview" />
      {slide.sound && (
        <div className="sound-badge" data-element="sound" title={t("mediaSound")}>
          <SoundIcon />
          <span>{mediaFileName(slide.sound.file)}</span>
        </div>
      )}
    </Stage>
  );
}

/** The quizmaster's notes of the slide, under the design area. */
export function NotesBox({ notes, onChange }: { notes: string; onChange: (notes: string) => void }) {
  const { t } = useApp();
  return <textarea className="notes-box" aria-label={t("notes")} placeholder={t("notesPlaceholder")} value={notes} onChange={(e) => onChange(e.target.value)} />;
}
