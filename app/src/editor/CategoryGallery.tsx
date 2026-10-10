import { useApp } from "../app-context";
import { SlideThumbnail } from "../host/SlideThumbnail";
import { NO_CONTEXT } from "../quiz";
import type { Category } from "../types";
import { sampleSlide } from "./categories";

export interface CategoryGalleryProps {
  categories: readonly Category[];
  /** The category every selected slide took on; null when they took on none or not the same. */
  current: string | null;
  /** Gives the selected slides the look of the category `id`. */
  apply: (id: string) => void;
}

/** The categories on the DESIGN tab, as an office suite's gallery: a click formats the selected slides like one (spec QZ-10). */
export function CategoryGallery({ categories, current, apply }: CategoryGalleryProps) {
  const { t } = useApp();
  if (categories.length === 0) return <p className="theme-missing">{t("noCategories")}</p>;
  return (
    <div className="theme-gallery">
      {categories.map((category) => (
        <button
          key={category.id}
          type="button"
          className="theme-tile"
          title={t("applyCategory", { name: category.name })}
          aria-pressed={category.id === current}
          onClick={() => apply(category.id)}
        >
          <SlideThumbnail className="theme-thumbnail" slide={sampleSlide(category)} index={0} count={1} context={NO_CONTEXT} />
          <span>{category.name || t("unnamedCategory")}</span>
        </button>
      ))}
    </div>
  );
}
