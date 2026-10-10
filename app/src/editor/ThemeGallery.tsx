import { useEffect } from "react";
import { useApp } from "../app-context";
import { useLatest } from "../hooks";
import { SlideThumbnail } from "../host/SlideThumbnail";
import { NO_CONTEXT } from "../quiz";
import { ThemeContext, useThemes } from "../theme/context";
import { isThemeMissing } from "../theme/themes";
import { GalleryStrip } from "./GalleryStrip";
import { DEFAULT_LAYOUT, slideFromLayout } from "./layouts";
import type { ThemeChoice } from "./useThemeChoice";

/** Each theme is drawn on an empty question, so its bars and answer colours show. */
const SAMPLE = slideFromLayout(DEFAULT_LAYOUT, "theme-sample");

/**
 * The themes to choose from in the ribbon, as in an office suite: pointing
 * at one previews it. A quiz whose theme is not installed is drawn in the
 * default one, and the gallery says so.
 */
export function ThemeGallery({ choice }: { choice: ThemeChoice }) {
  const { lang, t } = useApp();
  const themes = useThemes();
  const preview = useLatest(choice.preview);
  // The gallery can go away under the pointer (another tab), and so must its preview.
  useEffect(() => () => preview.current(null), [preview]);
  const missing = isThemeMissing(themes, choice.current);
  return (
    <>
      <GalleryStrip>
        {themes.map((theme) => (
          <button
            key={theme.id}
            type="button"
            className="theme-tile"
            title={theme.name[lang]}
            aria-pressed={theme.id === choice.current}
            onClick={() => choice.pick(theme.id)}
            onMouseEnter={() => choice.preview(theme.id)}
            onMouseLeave={() => choice.preview(null)}
            onFocus={() => choice.preview(theme.id)}
            onBlur={() => choice.preview(null)}
          >
            <ThemeContext value={theme}>
              <SlideThumbnail className="theme-thumbnail" slide={SAMPLE} index={0} count={1} context={NO_CONTEXT} />
            </ThemeContext>
            <span>{theme.name[lang]}</span>
          </button>
        ))}
      </GalleryStrip>
      {missing && <p className="theme-missing">{t("themeNotInstalled", { id: choice.current })}</p>}
    </>
  );
}
