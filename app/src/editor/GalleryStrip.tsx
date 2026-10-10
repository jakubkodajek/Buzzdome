import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useApp } from "../app-context";
import { ChevronLeftIcon, ChevronRightIcon } from "../icons";

/** Which ends of the strip are out of sight; both false when every tile shows. */
interface Hidden {
  before: boolean;
  after: boolean;
}

/**
 * A ribbon gallery's row of tiles (the themes). When the ribbon is too
 * narrow for all of them, the row scrolls a page at a time with the arrows
 * at its ends, as in an office suite, so the groups after it stay in sight.
 */
export function GalleryStrip({ children }: { children: ReactNode }) {
  const { t } = useApp();
  const row = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState<Hidden>({ before: false, after: false });
  const measure = useCallback(() => {
    const tiles = row.current;
    if (!tiles) return;
    // A pixel of slack for fractional widths at display scaling.
    const before = tiles.scrollLeft > 1;
    const after = tiles.scrollLeft + tiles.clientWidth < tiles.scrollWidth - 1;
    setHidden((was) => (was.before === before && was.after === after ? was : { before, after }));
  }, []);
  useLayoutEffect(() => {
    const tiles = row.current;
    if (!tiles) return;
    const observer = new ResizeObserver(measure);
    observer.observe(tiles);
    return () => observer.disconnect();
  }, [measure]);
  // Tiles that come and go (themes loading) do not resize the row itself.
  useLayoutEffect(measure);
  const page = (direction: 1 | -1) => row.current?.scrollBy({ left: direction * row.current.clientWidth, behavior: "smooth" });
  const scrolls = hidden.before || hidden.after;
  return (
    <div className="gallery-strip">
      {scrolls && (
        <button type="button" className="gallery-scroll" aria-label={t("galleryBack")} title={t("galleryBack")} disabled={!hidden.before} onClick={() => page(-1)}>
          <ChevronLeftIcon />
        </button>
      )}
      <div ref={row} className="theme-gallery" onScroll={measure}>
        {children}
      </div>
      {scrolls && (
        <button type="button" className="gallery-scroll" aria-label={t("galleryForward")} title={t("galleryForward")} disabled={!hidden.after} onClick={() => page(1)}>
          <ChevronRightIcon />
        </button>
      )}
    </div>
  );
}
