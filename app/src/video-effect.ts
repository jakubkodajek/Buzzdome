// How strong a question video's effect is drawn (spec QZ-4): at its
// strongest when the question starts, gone when the answer time is up.

/** Columns of blocks a mosaic has at full strength: the video is a blur of colour. */
const MOSAIC_COLUMNS = 12;
/** Blur at full strength, as a share of the video's width. */
const BLUR_SHARE = 0.04;

/**
 * Columns of blocks a mosaic over a video `width` pixels wide has at
 * `strength` (1 to 0); null once the blocks are no larger than a pixel.
 * The blocks shrink by the same ratio in equal steps of strength, so the
 * picture sharpens evenly to the eye.
 */
export function mosaicColumns(strength: number, width: number): number | null {
  if (width <= 0) return null;
  const columns = Math.round(MOSAIC_COLUMNS * (width / MOSAIC_COLUMNS) ** (1 - Math.min(1, Math.max(0, strength))));
  return columns >= width ? null : Math.max(1, columns);
}

/** Blur radius over a video drawn `width` pixels wide at `strength` (1 to 0), in pixels. */
export function blurRadius(strength: number, width: number): number {
  return Math.min(1, Math.max(0, strength)) * width * BLUR_SHARE;
}
