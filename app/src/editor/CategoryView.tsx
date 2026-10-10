import { useEffect, useRef, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { SlideThumbnail } from "../host/SlideThumbnail";
import { NO_CONTEXT } from "../quiz";
import { ThemeContext } from "../theme/context";
import type { Quiz, Theme } from "../types";
import { sampleSlide } from "./categories";
import { DesignArea } from "./DesignArea";
import type { CategoryEditor } from "./useCategories";

interface CategoryViewProps {
  quiz: Quiz;
  categories: CategoryEditor;
  /** The theme the canvas is drawn in: the quiz's, or the one pointed at in the theme gallery. */
  canvasTheme: Theme | null;
  /** Right click on a category, below them or on the canvas; the menu acts on the category shown. */
  onMenu: (event: MouseEvent) => void;
}

/**
 * The Categories view (spec QZ-10): the categories down the left side as
 * their sample slides, and the one shown in the middle, where its parts
 * are selected to set their look.
 */
export function CategoryView({ quiz, categories, canvasTheme, onMenu }: CategoryViewProps) {
  const { t } = useApp();
  const { category, index, actions } = categories;
  const selectedItem = useRef<HTMLLIElement>(null);
  useEffect(() => {
    selectedItem.current?.scrollIntoView({ block: "nearest" });
  }, [index]);
  const menu = (event: MouseEvent) => {
    event.preventDefault();
    onMenu(event);
  };

  return (
    <>
      <nav className="thumbnails category-list" aria-label={t("categories")} onContextMenu={menu}>
        <ol>
          {categories.categories.map((c, i) => (
            <li
              key={c.id}
              ref={i === index ? selectedItem : undefined}
              aria-selected={i === index}
              className={i === index ? "thumbnail selected" : "thumbnail"}
              onClick={() => categories.select(i)}
              onContextMenu={(e) => {
                e.stopPropagation();
                categories.select(i);
                menu(e);
              }}
            >
              <div className="thumbnail-frame">
                <SlideThumbnail className="thumbnail-stage" slide={sampleSlide(c)} index={0} count={1} context={NO_CONTEXT} />
                <span className="category-name">{c.name || t("unnamedCategory")}</span>
              </div>
            </li>
          ))}
        </ol>
        <button type="button" className="add-slide" title={actions.newCategory.label} aria-label={actions.newCategory.label} onClick={actions.newCategory.run}>
          +
        </button>
      </nav>
      <div className="studio-center">
        {category ? (
          <ThemeContext value={canvasTheme}>
            <DesignArea
              quiz={quiz}
              index={0}
              slide={sampleSlide(category)}
              context={NO_CONTEXT}
              element={categories.element}
              playingBackground={categories.playingBackground}
              onSelect={categories.selectPart}
              onEdit={categories.selectPart}
              onMenu={onMenu}
            />
          </ThemeContext>
        ) : (
          <div className="design-empty">
            <p>{t("noCategories")}</p>
            <p className="muted">{t("noCategoriesHint")}</p>
            <button type="button" onClick={actions.newCategory.run}>
              {actions.newCategory.label}
            </button>
          </div>
        )}
      </div>
    </>
  );
}
