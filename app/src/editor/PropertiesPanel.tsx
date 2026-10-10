import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import type { StringKey, Translate } from "../i18n";
import { CrossIcon, GamepadIcon, PaletteIcon, PictureIcon, QuestionIcon, SlideIcon, SoundIcon, StarIcon, VideoIcon } from "../icons";
import { readPreference, writePreference } from "../preferences";
import { SLIDE_TYPE_LABEL, slideTitle } from "../quiz";
import type { Slide } from "../types";
import type { Action } from "./actions";
import { ContextMenu, MixedContext } from "./controls";
import { answerIndex, type ElementId } from "./elements";
import { withRows, type Page, type PageId, type Section } from "./property-pages";
import {
  alphabetical,
  canReset,
  copiedValue,
  draggedValue,
  findMatch,
  pasteAction,
  resetValue,
  rowMatches,
  valueText,
  type CopiedValue,
  type PropertyRow,
  type TextPart,
} from "./property-rows";

const PAGE_TITLE: Record<PageId, StringKey> = {
  gameplay: "pageGameplay",
  scoring: "pageScoring",
  text: "pageText",
  media: "pageMedia",
  design: "pageDesign",
  advanced: "pageAdvanced",
};

const PAGE_ICON: Record<PageId, ReactNode> = {
  gameplay: <GamepadIcon />,
  scoring: <StarIcon />,
  text: <span className="text-icon">T</span>,
  media: <PictureIcon />,
  design: <PaletteIcon />,
  advanced: <span className="text-icon">{"{}"}</span>,
};

// The panel's own settings, kept per computer.
const DESCRIPTIONS_KEY = "quiz-show.properties.descriptions";
const DARK_KEY = "quiz-show.properties.dark";
const CLASSIC_KEY = "quiz-show.properties.classic";

/** A setting of the panel kept per computer, `initial` until changed. */
function useStoredFlag(key: string, initial: boolean): [boolean, (on: boolean) => void] {
  const [on, setOn] = useState(() => {
    const stored = readPreference(key);
    return stored === null ? initial : stored === "true";
  });
  return [
    on,
    (next) => {
      setOn(next);
      writePreference(key, String(next));
    },
  ];
}

interface PropertiesPanelProps {
  /** The slide on the canvas. */
  slide: Slide;
  /** How many slides are selected; a change goes to all of them. */
  count: number;
  /** The part of the slide selected on the canvas (one slide only). */
  element: ElementId;
  /** The selection's pages and the properties the slides differ in (`selectionPages`). */
  pages: Page[];
  mixed: ReadonlySet<string>;
  /** The value last copied from a property, to paste into another. */
  copied: CopiedValue | null;
  onCopy: (value: CopiedValue) => void;
  /** Ctrl+F asked for the search: it is focused, and `onSearchFocused` called. */
  focusSearch: boolean;
  onSearchFocused: () => void;
}

/**
 * The properties of the selected slides, or of the part of a slide selected
 * on the canvas, in pages of foldable sections (spec QZ-1). For several
 * slides it shows what they all have, values they differ in as Mixed, and a
 * change sets every one. The search looks through all pages; a number
 * changes by dragging its name; a right click resets, copies or pastes a
 * value; the … menu folds, shows descriptions, darkens the panel or lists
 * every property by name.
 */
export function PropertiesPanel({ slide, count, element, pages, mixed, copied, onCopy, focusSearch, onSearchFocused }: PropertiesPanelProps) {
  const { t, lang } = useApp();
  const search = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!focusSearch) return;
    search.current?.select();
    onSearchFocused();
  }, [focusSearch, onSearchFocused]);

  const [pageId, setPageId] = useState<PageId>("gameplay");
  const [query, setQuery] = useState("");
  const [folded, setFolded] = useState<ReadonlySet<StringKey>>(new Set());
  const [more, setMore] = useState<ReadonlySet<StringKey>>(new Set());
  const [described, setDescribed] = useState<PropertyRow | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; row: PropertyRow | null } | null>(null);
  const [descriptions, setDescriptions] = useStoredFlag(DESCRIPTIONS_KEY, true);
  const [dark, setDark] = useStoredFlag(DARK_KEY, false);
  const [classic, setClassic] = useStoredFlag(CLASSIC_KEY, false);
  const page = pages.find((p) => p.id === pageId) ?? pages[0];
  const header = count > 1 ? severalHeader(t, count) : panelHeader(t, slide, element);
  const needle = query.trim();

  const sections: Section[] = needle
    ? withRows(pages, (row) => rowMatches(row, needle)).flatMap((p) => p.sections)
    : classic
      ? [{ title: "classicList", rows: alphabetical(pages.flatMap((p) => p.sections.map((s) => ({ name: t(s.title), rows: s.rows }))), lang) }]
      : (page?.sections ?? []);

  const toggle = (set: (change: (current: ReadonlySet<StringKey>) => ReadonlySet<StringKey>) => void, title: StringKey) =>
    set((current) => {
      const next = new Set(current);
      if (!next.delete(title)) next.add(title);
      return next;
    });

  const optionsMenu: (Action | "separator")[] = [
    { label: t("expandAll"), disabled: classic, run: () => setFolded(new Set()) },
    { label: t("collapseAll"), disabled: classic, run: () => setFolded(new Set(pages.flatMap((p) => p.sections.map((s) => s.title)))) },
    "separator",
    { label: t("showDescriptions"), checked: descriptions, run: () => setDescriptions(!descriptions) },
    { label: t("darkPanel"), checked: dark, run: () => setDark(!dark) },
    { label: t("classicList"), checked: classic, run: () => setClassic(!classic) },
  ];
  const rowMenu = (row: PropertyRow): Action[] => {
    const value = copiedValue(row);
    const paste = copied && pasteAction(row, copied);
    return [
      { label: t("resetValue"), disabled: !canReset(row), run: () => resetValue(row) },
      {
        label: t("copyValue"),
        disabled: !value,
        run: () => {
          if (!value) return;
          onCopy(value);
          // Other programs get the value as text; pasting here works without a clipboard too.
          navigator.clipboard?.writeText(valueText(value)).catch(() => undefined);
        },
      },
      { label: t("pasteValue"), disabled: !paste, run: () => paste?.() },
    ];
  };

  const searchKeys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Escape") return;
    // Esc leaves the search; it is not the editor's Esc (back to the whole slide).
    e.preventDefault();
    e.stopPropagation();
    if (query) setQuery("");
    else e.currentTarget.blur();
  };

  const renderRow = (row: PropertyRow) => {
    const descriptionMatch = needle !== "" && findMatch(row.label, needle) === null ? findMatch(row.description, needle) : null;
    return (
      <div
        key={row.id}
        className={row.wide ? "props-row wide" : "props-row"}
        title={descriptions ? undefined : row.description}
        onMouseEnter={() => setDescribed(row)}
        onFocusCapture={() => setDescribed(row)}
        onContextMenu={(e) => {
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY, row });
        }}
      >
        <RowLabel row={row} needle={needle} />
        <span className="props-control">
          <MixedContext value={mixed.has(row.id)}>{row.control}</MixedContext>
        </span>
        {descriptionMatch && (
          <small className="props-match">
            <Highlighted parts={descriptionMatch} />
          </small>
        )}
      </div>
    );
  };

  return (
    <aside className={dark ? "properties-panel dark" : "properties-panel"}>
      <header className="props-header">
        <span className="props-icon">{header.icon}</span>
        <div className="props-heading">
          <div className="props-title">
            <b>{header.title}</b>
            {header.chip && <span className="chip">{header.chip}</span>}
          </div>
          <small>{header.subtitle || " "}</small>
        </div>
        <button
          type="button"
          className="props-options"
          title={t("panelOptions")}
          aria-label={t("panelOptions")}
          aria-haspopup="menu"
          onClick={(e) => {
            const box = e.currentTarget.getBoundingClientRect();
            setMenu({ x: box.left, y: box.bottom, row: null });
          }}
        >
          …
        </button>
      </header>
      <div className="props-search">
        <input
          ref={search}
          type="search"
          placeholder={t("searchProperties")}
          aria-label={t("searchProperties")}
          title={`${t("searchProperties")} (Ctrl+F)`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={searchKeys}
        />
        {query && (
          <button type="button" className="mini" title={t("clearSearch")} aria-label={t("clearSearch")} onClick={() => setQuery("")}>
            <CrossIcon />
          </button>
        )}
      </div>
      {!needle && classic && (
        <div className="props-classic-bar">
          <span>{t("classicList")}</span>
          <button type="button" className="props-button" onClick={() => setClassic(false)}>
            {t("categorizedView")}
          </button>
        </div>
      )}
      {!needle && !classic && pages.length > 1 && (
        <nav className="props-pages" role="tablist">
          {pages.map((p) => (
            <button
              key={p.id}
              type="button"
              role="tab"
              aria-selected={p === page}
              className={p === page ? "active" : ""}
              title={t(PAGE_TITLE[p.id])}
              onClick={() => setPageId(p.id)}
            >
              {PAGE_ICON[p.id]}
              {p === page && <span>{t(PAGE_TITLE[p.id])}</span>}
            </button>
          ))}
        </nav>
      )}
      <div className="props-body">
        {sections.length === 0 && <p className="props-empty">{needle ? t("noMatch") : t("noCommonProperties")}</p>}
        {sections.map((section) => {
          if (classic && !needle) return <div key={section.title}>{section.rows.map(renderRow)}</div>;
          const open = needle !== "" || !folded.has(section.title);
          const rare = section.rows.some((row) => row.rare);
          const showsAll = needle !== "" || more.has(section.title);
          return (
            <section key={section.title} className="props-section">
              <button type="button" className="props-section-title" aria-expanded={open} onClick={() => toggle(setFolded, section.title)}>
                <span className={open ? "fold open" : "fold"}>›</span>
                {t(section.title)}
              </button>
              {open && section.rows.filter((row) => showsAll || !row.rare).map(renderRow)}
              {open && rare && !needle && (
                <button type="button" className="props-more" aria-expanded={showsAll} onClick={() => toggle(setMore, section.title)}>
                  {t(showsAll ? "fewerOptions" : "moreOptions")}
                </button>
              )}
            </section>
          );
        })}
      </div>
      {descriptions && (
        <footer className="props-description">
          {described ? (
            <>
              <b>{described.label}</b>
              <p>{described.description}</p>
            </>
          ) : (
            <p className="muted">{t("describeHint")}</p>
          )}
        </footer>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.row ? rowMenu(menu.row) : optionsMenu} onClose={() => setMenu(null)} />}
    </aside>
  );
}

/** A row's name, with the search match marked; a number's name drags it (Shift: faster, Alt: slower). */
function RowLabel({ row, needle }: { row: PropertyRow; needle: string }) {
  const { t } = useApp();
  const number = row.value?.kind === "number" ? row.value : null;
  const parts = needle ? findMatch(row.label, needle) : null;
  const startDrag = (e: ReactPointerEvent<HTMLSpanElement>) => {
    if (!number || e.button !== 0) return;
    e.preventDefault();
    const label = e.currentTarget;
    const [startX, start] = [e.clientX, number.value];
    let last = start;
    label.setPointerCapture(e.pointerId);
    const move = (event: PointerEvent) => {
      const value = draggedValue(start, event.clientX - startX, number, { shift: event.shiftKey, alt: event.altKey });
      if (value === last) return;
      last = value;
      number.set(value);
    };
    const end = () => {
      label.removeEventListener("pointermove", move);
      label.removeEventListener("pointerup", end);
      label.removeEventListener("pointercancel", end);
    };
    label.addEventListener("pointermove", move);
    label.addEventListener("pointerup", end);
    label.addEventListener("pointercancel", end);
  };
  return (
    <span className={number ? "props-label draggable" : "props-label"} title={number ? t("dragToChange") : undefined} onPointerDown={startDrag}>
      {parts ? <Highlighted parts={parts} /> : row.label}
    </span>
  );
}

function Highlighted({ parts }: { parts: TextPart[] }) {
  return (
    <>
      {parts.map((part, i) => (part.match ? <mark key={i}>{part.text}</mark> : <span key={i}>{part.text}</span>))}
    </>
  );
}

const ELEMENT_TITLE: Record<"question" | "title" | "text", StringKey> = {
  question: "questionField",
  title: "slideTitleField",
  text: "slideTextField",
};

interface Header {
  icon: ReactNode;
  title: string;
  chip?: string;
  subtitle: string;
}

function severalHeader(t: Translate, count: number): Header {
  return { icon: <SlideIcon />, title: t("slidesSelected", { n: count }), subtitle: t("commonPropertiesHint") };
}

function panelHeader(t: Translate, slide: Slide, element: ElementId): Header {
  const answer = answerIndex(element);
  if (answer !== null && slide.type === "question") {
    return { icon: <QuestionIcon />, title: t("elementAnswer", { letter: ANSWER_LETTERS[answer] }), subtitle: slide.answers[answer] ?? "" };
  }
  switch (element) {
    case "question":
    case "title":
    case "text":
      return { icon: <span className="text-icon">T</span>, title: t(ELEMENT_TITLE[element]), subtitle: slideTitle(slide) };
    case "picture":
      return { icon: <PictureIcon />, title: t("mediaPicture"), subtitle: slide.picture ?? "" };
    case "video":
      return { icon: <VideoIcon />, title: t("mediaVideo"), subtitle: slide.video?.file ?? "" };
    case "sound":
      return { icon: <SoundIcon />, title: t("mediaSound"), subtitle: slide.sound?.file ?? "" };
    default:
      return {
        icon: slide.type === "question" ? <QuestionIcon /> : <SlideIcon />,
        title: t(SLIDE_TYPE_LABEL[slide.type]),
        chip: slide.type === "question" ? t(slide.mode === "everyone" ? "everyone" : "fastestFinger") : undefined,
        subtitle: slideTitle(slide),
      };
  }
}
