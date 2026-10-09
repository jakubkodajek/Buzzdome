import { memo } from "react";
import { SlideBackground } from "../slide-media";
import type { Slide } from "../types";
import { previewView } from "./preview";
import { SlideScreen } from "./SlideScreen";
import { Stage } from "./Stage";

/**
 * A slide drawn small, as the audience will see it (styled by `className`,
 * which sets its size). Drawn again only when the slide or its place changes.
 */
export const SlideThumbnail = memo(function SlideThumbnail(props: {
  className: string;
  slide: Slide;
  index: number;
  count: number;
  /** Its number among the questions; null for other slides. */
  questionNumber: number | null;
  questionCount: number;
}) {
  const { className, slide, index, count, questionNumber, questionCount } = props;
  const position = questionNumber === null ? null : { n: questionNumber, total: questionCount };
  return (
    <Stage className={className}>
      <SlideBackground slide={slide} mode="thumbnail" />
      <SlideScreen view={previewView(slide, index, count, "")} position={position} clock={null} mode="thumbnail" />
    </Stage>
  );
});
