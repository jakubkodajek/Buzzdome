import { useEffect } from "react";
import { useApp } from "../app-context";
import { useLatest } from "../hooks";
import { SlideThumbnail } from "../host/SlideThumbnail";
import { NO_CONTEXT } from "../quiz";
import { ThemeContext } from "../theme/context";
import { BUILTIN_THEMES, resolveTheme, type Theme } from "../theme/themes";
import { DEFAULT_LAYOUT, slideFromLayout } from "./layouts";

/** Each theme is drawn on an empty question, so its bars and answer colours show. */
const SAMPLE = slideFromLayout(DEFAULT_LAYOUT, "theme-sample");
const THEMES: readonly Theme[] = BUILTIN_THEMES.map((theme) => resolveTheme(theme.id));

interface ThemeGalleryProps {
  /** Id of the quiz's theme. */
  current: string;
  onPick: (id: string) => void;
  /** Called with the theme under the pointer or focus, null when there is none. */
  onPreview: (id: string | null) => void;
}

/** The themes to choose from in the ribbon, as in an office suite: pointing at one previews it. */
export function ThemeGallery({ current, onPick, onPreview }: ThemeGalleryProps) {
  const { lang } = useApp();
  const preview = useLatest(onPreview);
  // The gallery can go away under the pointer (another tab), and so must its preview.
  useEffect(() => () => preview.current(null), [preview]);
  return (
    <div className="theme-gallery">
      {THEMES.map((theme) => (
        <button
          key={theme.id}
          type="button"
          className="theme-tile"
          title={theme.name[lang]}
          aria-pressed={theme.id === current}
          onClick={() => onPick(theme.id)}
          onMouseEnter={() => onPreview(theme.id)}
          onMouseLeave={() => onPreview(null)}
          onFocus={() => onPreview(theme.id)}
          onBlur={() => onPreview(null)}
        >
          <ThemeContext value={theme}>
            <SlideThumbnail className="theme-thumbnail" slide={SAMPLE} index={0} count={1} context={NO_CONTEXT} />
          </ThemeContext>
          <span>{theme.name[lang]}</span>
        </button>
      ))}
    </div>
  );
}
