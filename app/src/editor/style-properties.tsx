// A slide's own look over its theme (quiz_core::style) as property rows:
// the gradient behind it, and the font, colours, shadow and fill of each of
// its texts. Only what differs from the theme is stored.

import type { StringKey, Translate } from "../i18n";
import { THEME_TEXT, type StyledText } from "../theme/slide-style";
import type { Slide, TextAlign, TextStyle } from "../types";
import type { ElementId } from "./elements";
import type { SlideEditing } from "./properties";
import type { PropertyRow } from "./property-rows";
import { rowBuilders } from "./row-builders";
import { setSlideFill, setTextStyle, type StyledPart } from "./slides";

/** The texts of a slide that have a look of their own. */
export function styledParts(slide: Slide): StyledPart[] {
  switch (slide.type) {
    case "question":
      return ["headline", "answers"];
    case "billboard":
      return ["headline", "text"];
    case "explanation":
      return ["text"];
    case "endOfRound":
    case "final":
      return ["headline"];
  }
}

/** The text whose look a part of the slide selected on the canvas sets; null for a picture, a sound or the slide. */
export function styledPartOf(element: ElementId): StyledPart | null {
  if (element.startsWith("answer-")) return "answers";
  switch (element) {
    case "question":
    case "title":
      return "headline";
    case "text":
      return "text";
    default:
      return null;
  }
}

/** The section title of a part's look. */
export function lookTitle(slide: Slide, part: StyledPart): StringKey {
  switch (part) {
    case "headline":
      return slide.type === "question" ? "sectionQuestionLook" : "sectionTitleLook";
    case "text":
      return "sectionTextLook";
    case "answers":
      return "sectionAnswersLook";
  }
}

/** How slide.css draws the part's text. */
function drawnAs(slide: Slide, part: StyledPart): StyledText {
  switch (part) {
    case "headline":
      return slide.type === "question" ? "question" : "title";
    case "text":
      return "text";
    case "answers":
      return "answer";
  }
}

/** The slide's look as rows: `fill` behind the slide, `look` of one of its texts. */
export function styleProperties(t: Translate, { slide, update }: Pick<SlideEditing, "slide" | "update">) {
  const { choiceRow, colorRow, flagRow, fontRow, gradientRow, numberRow } = rowBuilders(t);
  const style = slide.style;

  const look = (part: StyledPart): PropertyRow[] => {
    const own = style?.[part];
    const theme = THEME_TEXT[drawnAs(slide, part)];
    const id = (name: string) => `${part}.${name}`;
    const set = (change: Partial<TextStyle>, field?: string) => update((s) => setTextStyle(s, part, change), field);
    const aligns: [TextAlign | "theme", string][] = [
      ["theme", t("alignTheme")],
      ["left", t("alignLeft")],
      ["center", t("alignCenter")],
      ["right", t("alignRight")],
    ];
    return [
      fontRow(id("font"), "propFont", "descFont", {
        value: { font: own?.font, size: own?.size, bold: own?.bold, italic: own?.italic, underline: own?.underline },
        themeBold: theme.bold,
        change: (change) => set(change, id("font")),
      }),
      colorRow(id("color"), "propTextColor", "descTextColor", {
        value: own?.color ?? null,
        initial: null,
        none: "themeColor",
        set: (color) => set({ color: color ?? undefined }, id("color")),
      }),
      choiceRow(id("align"), "propAlign", "descAlign", {
        value: own?.align ?? "theme",
        initial: "theme",
        options: aligns,
        set: (align) => set({ align: align === "theme" ? undefined : align }),
      }),
      flagRow(id("shadow"), "propShadow", "descShadow", {
        value: own?.shadow ?? theme.shadow,
        initial: theme.shadow,
        set: (shadow) => set({ shadow: shadow === theme.shadow ? undefined : shadow }),
      }),
      colorRow(
        id("shadowColor"),
        "propShadowColor",
        "descShadowColor",
        { value: own?.shadowColor ?? null, initial: null, none: "themeColor", set: (color) => set({ shadowColor: color ?? undefined }, id("shadowColor")) },
        { rare: true },
      ),
      gradientRow(id("fill"), "propGradient", part === "answers" ? "descAnswerGradient" : "descTextGradient", {
        value: own?.fill ?? null,
        change: (change, field) => update((s) => setTextStyle(s, part, { fill: change(s.style?.[part]?.fill ?? null) ?? undefined }), field),
      }),
      numberRow(
        id("transparency"),
        "propTransparency",
        part === "answers" ? "descAnswerTransparency" : "descTextTransparency",
        {
          value: own?.transparency ?? 0,
          initial: 0,
          min: 0,
          max: 100,
          unit: "%",
          slider: true,
          set: (transparency) => set({ transparency: transparency === 0 ? undefined : transparency }, id("transparency")),
        },
        { rare: true },
      ),
    ];
  };

  return {
    fill: gradientRow("fill", "propGradient", "descSlideGradient", {
      value: style?.fill ?? null,
      change: (change, field) => update((s) => setSlideFill(s, change(s.style?.fill ?? null)), field),
    }),
    look,
  };
}

export type StyleProperties = ReturnType<typeof styleProperties>;
