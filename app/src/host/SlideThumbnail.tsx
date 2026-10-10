import { memo } from "react";
import { sameContext, type SlideContext } from "../quiz";
import { screenOf } from "../theme/themes";
import type { Slide } from "../types";
import { previewView } from "./preview";
import { SlideBoundary } from "./SlideBoundary";
import { SlideScreen } from "./SlideScreen";
import { Stage } from "./Stage";

interface SlideThumbnailProps {
  className: string;
  slide: Slide;
  index: number;
  count: number;
  context: SlideContext;
}

/**
 * A slide drawn small, as the audience will see it (styled by `className`,
 * which sets its size). Drawn again only when what it shows changes.
 */
export const SlideThumbnail = memo(function SlideThumbnail(props: SlideThumbnailProps) {
  return (
    <SlideBoundary className={props.className} slide={props.slide}>
      <ThumbnailStage {...props} />
    </SlideBoundary>
  );
}, sameThumbnail);

function ThumbnailStage({ className, slide, index, count, context }: SlideThumbnailProps) {
  const view = previewView(slide, index, count, "");
  return (
    <Stage className={className} screen={screenOf(view)} slide={slide} mode="thumbnail">
      <SlideScreen view={view} context={context} clock={null} mode="thumbnail" />
    </Stage>
  );
}

function sameThumbnail(a: SlideThumbnailProps, b: SlideThumbnailProps): boolean {
  return (
    a.className === b.className && a.slide === b.slide && a.index === b.index && a.count === b.count && sameContext(a.context, b.context)
  );
}
