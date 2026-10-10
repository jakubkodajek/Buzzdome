import {
  Fragment,
  useEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import type { StringKey, Translate } from "../i18n";
import { CrossIcon, GamepadIcon, PaletteIcon, PictureIcon, QuestionIcon, SlideIcon, SoundIcon, StarIcon, VideoIcon } from "../icons";
import { readPreference, writePreference } from "../preferences";
import { SLIDE_TYPE_LABEL, slideTitle } from "../quiz";
import type { Category, Slide } from "../types";
import { keyCombo, type Action } from "./actions";
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
  pasteText,
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
  /** What the properties are of (`selectionHeader`, `categoryHeader`). */
  header: PanelHeader;
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
 * The properties of the selected slides, of the part of a slide selected
 * on the canvas, or of a category, in pages of foldable sections (spec QZ-1). For several
 * slides it shows what they all have, values they differ in as Mixed, and a
 * change sets every one. The search looks through all pages; a number
 * changes by dragging its name; a right click resets, copies or pastes a
 * value; the … menu folds, shows descriptions, darkens the panel or lists
 * every property by name.
 */
export function PropertiesPanel({ header, pages, mixed, copied, onCopy, focusSearch, onSearchFocused }: PropertiesPanelProps) {
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
  const [openParts, setOpenParts] = useState<ReadonlySet<string>>(new Set());
  const [described, setDescribed] = useState<PropertyRow | null>(null);
  // A row whose Ctrl+V waits for the web view's paste event.
  const pasteWaiting = useRef<PropertyRow | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; row: PropertyRow | null } | null>(null);
  const [descriptions, setDescriptions] = useStoredFlag(DESCRIPTIONS_KEY, true);
  const [dark, setDark] = useStoredFlag(DARK_KEY, false);
  const [classic, setClassic] = useStoredFlag(CLASSIC_KEY, false);
  const page = pages.find((p) => p.id === pageId) ?? pages[0];
  const needle = query.trim();

  const matches = (row: PropertyRow) => rowMatches(row, needle) || (row.parts ?? []).some((part) => rowMatches(part, needle));
  const sections: Section[] = needle
    ? withRows(pages, matches).flatMap((p) => p.sections)
    : classic
      ? [{ title: "classicList", rows: alphabetical(pages.flatMap((p) => p.sections.map((s) => ({ name: t(s.title), rows: s.rows }))), lang) }]
      : (page?.sections ?? []);

  const toggle = <T,>(set: (change: (current: ReadonlySet<T>) => ReadonlySet<T>) => void, item: T) =>
    set((current) => {
      const next = new Set(current);
      if (!next.delete(item)) next.add(item);
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
  const copy = (row: PropertyRow) => {
    const value = copiedValue(row);
    if (!value) return;
    onCopy(value);
    // Other programs get the value as text (a colour as #1E4A6B); pasting here works without a clipboard too.
    navigator.clipboard?.writeText(valueText(value)).catch(() => undefined);
  };
  /** Pastes the clipboard's text, or the value copied here while the clipboard still holds it (or cannot be read). */
  const paste = (row: PropertyRow, text: string | null) => {
    const own = copied && (text === null || text === valueText(copied)) ? pasteAction(row, copied) : null;
    (own ?? (text === null ? null : pasteText(row, text)))?.();
  };
  const rowMenu = (row: PropertyRow): (Action | "separator")[] => {
    const pasted = copied && pasteAction(row, copied);
    return [
      { label: t("resetValue"), disabled: !canReset(row), run: () => resetValue(row) },
      { label: t("copyValue"), keys: ["Ctrl+C"], disabled: !row.value, run: () => copy(row) },
      { label: t("pasteValue"), keys: ["Ctrl+V"], disabled: !pasted, run: () => pasted?.() },
      ...(row.menu ? ["separator" as const, ...row.menu] : []),
    ];
  };
  // Ctrl+C and Ctrl+V copy and paste the value of the property being worked in, not slides; a text field keeps its own.
  const rowKeys = (row: PropertyRow) => (e: KeyboardEvent<HTMLDivElement>) => {
    const combo = keyCombo(e);
    if ((combo !== "Ctrl+C" && combo !== "Ctrl+V") || !row.value || editsText(e.target)) return;
    e.stopPropagation();
    if (combo === "Ctrl+C") {
      e.preventDefault();
      copy(row);
      return;
    }
    // The web view follows with a paste event that has the clipboard's text; where none comes, the value copied here is pasted.
    pasteWaiting.current = row;
    setTimeout(() => {
      if (pasteWaiting.current !== row) return;
      pasteWaiting.current = null;
      paste(row, null);
    });
  };
  const rowPaste = (row: PropertyRow) => (e: ClipboardEvent<HTMLDivElement>) => {
    if (!row.value || editsText(e.target)) return;
    e.preventDefault();
    pasteWaiting.current = null;
    paste(row, e.clipboardData.getData("text"));
  };

  const searchKeys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Escape") return;
    // Esc leaves the search; it is not the editor's Esc (back to the whole slide).
    e.preventDefault();
    e.stopPropagation();
    if (query) setQuery("");
    else e.currentTarget.blur();
  };

  const renderRow = (row: PropertyRow, part = false): ReactNode => {
    const descriptionMatch = needle !== "" && findMatch(row.label, needle) === null ? findMatch(row.description, needle) : null;
    const parts = row.parts ?? [];
    const partsOpen = parts.length > 0 && (needle !== "" || openParts.has(row.id));
    return (
      <Fragment key={row.id}>
        <div
          className={["props-row", row.wide && "wide", part && "part"].filter(Boolean).join(" ")}
          title={descriptions ? undefined : row.description}
          onMouseEnter={() => setDescribed(row)}
          onFocusCapture={() => setDescribed(row)}
          onContextMenu={(e) => {
            e.preventDefault();
            setMenu({ x: e.clientX, y: e.clientY, row });
          }}
          onKeyDown={rowKeys(row)}
          onPaste={rowPaste(row)}
        >
          <RowLabel row={row} needle={needle} fold={parts.length > 0 ? { open: partsOpen, toggle: () => toggle(setOpenParts, row.id) } : undefined} />
          <span className="props-control">
            <MixedContext value={mixed.has(row.id)}>{row.control}</MixedContext>
          </span>
          {descriptionMatch && (
            <small className="props-match">
              <Highlighted parts={descriptionMatch} />
            </small>
          )}
        </div>
        {partsOpen && parts.map((p) => renderRow(p, true))}
      </Fragment>
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
          if (classic && !needle) return <div key={section.title}>{section.rows.map((row) => renderRow(row))}</div>;
          const open = needle !== "" || !folded.has(section.title);
          const rare = section.rows.some((row) => row.rare);
          const showsAll = needle !== "" || more.has(section.title);
          return (
            <section key={section.title} className="props-section">
              <button type="button" className="props-section-title" aria-expanded={open} onClick={() => toggle(setFolded, section.title)}>
                <span className={open ? "fold open" : "fold"}>›</span>
                {t(section.title)}
              </button>
              {open && section.rows.filter((row) => showsAll || !row.rare).map((row) => renderRow(row))}
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

/** Whether a key press or a paste is for the text of a field rather than for its property. */
function editsText(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) return !["range", "checkbox", "radio", "button"].includes(target.type);
  return target instanceof HTMLElement && target.isContentEditable;
}

/**
 * A row's name, with the search match marked. A number's name drags it
 * (Shift: faster, Alt: slower); a grouped property's name folds its parts
 * open (`fold`).
 */
function RowLabel({ row, needle, fold }: { row: PropertyRow; needle: string; fold?: { open: boolean; toggle: () => void } }) {
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
  const text = parts ? <Highlighted parts={parts} /> : row.label;
  if (fold) {
    return (
      <button type="button" className="props-label props-fold" aria-expanded={fold.open} onClick={fold.toggle}>
        <span className={fold.open ? "fold open" : "fold"}>›</span>
        {text}
      </button>
    );
  }
  return (
    <span className={number ? "props-label draggable" : "props-label"} title={number ? t("dragToChange") : undefined} onPointerDown={startDrag}>
      {text}
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

export interface PanelHeader {
  icon: ReactNode;
  title: string;
  chip?: string;
  subtitle: string;
}

/** The header for the selected slides: the slide on the canvas, or the part of it selected (`element`), or how many are selected. */
export function selectionHeader(t: Translate, slide: Slide, count: number, element: ElementId): PanelHeader {
  if (count > 1) return { icon: <SlideIcon />, title: t("slidesSelected", { n: count }), subtitle: t("commonPropertiesHint") };
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

/** The header for a category in the Categories view, with how many slides took it on. */
export function categoryHeader(t: Translate, category: Category, slides: number): PanelHeader {
  return { icon: <PaletteIcon />, title: category.name || t("unnamedCategory"), chip: t("categorySlides", { n: slides }), subtitle: t("categoryHint") };
}
