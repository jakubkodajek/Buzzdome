import { useState, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import type { StringKey, Translate } from "../i18n";
import { GamepadIcon, PaletteIcon, PictureIcon, QuestionIcon, SlideIcon, SoundIcon, StarIcon, VideoIcon } from "../icons";
import { showsRanking, SLIDE_TYPE_LABEL, slideTitle } from "../quiz";
import type { Slide } from "../types";
import { answerIndex, type ElementId } from "./elements";
import { slideProperties, type PropertyRow, type SlideEditing, type SlideProperties } from "./properties";

type PageId = "gameplay" | "scoring" | "text" | "design" | "advanced";

interface Section {
  title: StringKey;
  rows: PropertyRow[];
}

interface Page {
  id: PageId;
  sections: Section[];
}

const PAGE_TITLE: Record<PageId, StringKey> = {
  gameplay: "pageGameplay",
  scoring: "pageScoring",
  text: "pageText",
  design: "pageDesign",
  advanced: "pageAdvanced",
};

const PAGE_ICON: Record<PageId, ReactNode> = {
  gameplay: <GamepadIcon />,
  scoring: <StarIcon />,
  text: <span className="text-icon">T</span>,
  design: <PaletteIcon />,
  advanced: <span className="text-icon">{"{}"}</span>,
};

/**
 * The properties of the selected slide, or of the part of it selected on the
 * canvas, in pages of foldable sections. The search looks through all pages;
 * the property under the pointer is described at the bottom.
 */
export function PropertiesPanel({ editing, element }: { editing: SlideEditing; element: ElementId }) {
  const { t } = useApp();
  const pages = propertyPages(editing.slide, element, slideProperties(t, editing));
  const [pageId, setPageId] = useState<PageId>("gameplay");
  const [query, setQuery] = useState("");
  const [folded, setFolded] = useState<ReadonlySet<StringKey>>(new Set());
  const [described, setDescribed] = useState<PropertyRow | null>(null);
  const page = pages.find((p) => p.id === pageId) ?? pages[0];
  const header = panelHeader(t, editing.slide, element);

  const needle = query.trim().toLowerCase();
  const sections: Section[] = needle
    ? pages
        .flatMap((p) => p.sections)
        .map((section) => ({ ...section, rows: section.rows.filter((row) => `${row.label} ${row.description}`.toLowerCase().includes(needle)) }))
        .filter((section) => section.rows.length > 0)
    : page.sections;

  const toggle = (title: StringKey) =>
    setFolded((current) => {
      const next = new Set(current);
      if (!next.delete(title)) next.add(title);
      return next;
    });

  return (
    <aside className="properties-panel">
      <header className="props-header">
        <span className="props-icon">{header.icon}</span>
        <div className="props-heading">
          <div className="props-title">
            <b>{header.title}</b>
            {header.chip && <span className="chip">{header.chip}</span>}
          </div>
          <small>{header.subtitle || " "}</small>
        </div>
      </header>
      <input className="props-search" type="search" placeholder={t("searchProperties")} value={query} onChange={(e) => setQuery(e.target.value)} />
      {!needle && pages.length > 1 && (
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
        {sections.length === 0 && <p className="props-empty">{t("noMatch")}</p>}
        {sections.map((section) => {
          const open = needle !== "" || !folded.has(section.title);
          return (
            <section key={section.title} className="props-section">
              <button type="button" className="props-section-title" aria-expanded={open} onClick={() => toggle(section.title)}>
                <span className={open ? "fold open" : "fold"}>›</span>
                {t(section.title)}
              </button>
              {open &&
                section.rows.map((row) => (
                  <div
                    key={row.label}
                    className={row.wide ? "props-row wide" : "props-row"}
                    onMouseEnter={() => setDescribed(row)}
                    onFocusCapture={() => setDescribed(row)}
                  >
                    <span className="props-label">{row.label}</span>
                    <span className="props-control">{row.control}</span>
                  </div>
                ))}
            </section>
          );
        })}
      </div>
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
    </aside>
  );
}

const ELEMENT_TITLE: Record<"question" | "title" | "text", StringKey> = {
  question: "questionField",
  title: "slideTitleField",
  text: "slideTextField",
};

function panelHeader(t: Translate, slide: Slide, element: ElementId): { icon: ReactNode; title: string; chip?: string; subtitle: string } {
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

/** The pages for the selected part of the slide. */
function propertyPages(slide: Slide, element: ElementId, rows: SlideProperties): Page[] {
  const page = (id: PageId, sections: Section[]): Page => ({ id, sections });
  const answer = answerIndex(element);

  if (answer !== null && slide.type === "question") {
    return [page("text", [{ title: "sectionAnswer", rows: [rows.answerText(slide, answer), rows.isCorrect(slide, answer), rows.removeAnswer(slide, answer)] }])];
  }
  if (element === "question" && slide.type === "question") return [page("text", [{ title: "sectionText", rows: [rows.questionText(slide)] }])];
  if ((element === "title" || element === "text") && slide.type !== "question") return [page("text", [{ title: "sectionText", rows: textRows(slide, rows) }])];
  if (element === "picture") return [page("design", [{ title: "sectionPictureVideo", rows: [rows.picture] }])];
  if (element === "video") return [page("design", [{ title: "sectionPictureVideo", rows: videoRows(rows) }])];
  if (element === "sound") return [page("design", [{ title: "sectionSound", rows: soundRows(rows) }])];

  // The slide itself.
  const pages: Page[] =
    slide.type === "question"
      ? [
          page("gameplay", [
            { title: "sectionSlide", rows: [rows.type] },
            { title: "sectionTiming", rows: [rows.answerTime(slide)] },
            { title: "sectionAnswering", rows: [rows.everyoneAnswers(slide), rows.answerCount(slide), rows.correctAnswer(slide)] },
          ]),
          page("scoring", [
            {
              title: "sectionPoints",
              rows: [rows.pointsStart(slide), rows.falling(slide), ...(slide.pointsEnd !== null ? [rows.pointsEnd(slide)] : []), rows.penalty(slide)],
            },
          ]),
          page("text", [{ title: "sectionText", rows: [rows.questionText(slide), rows.notes] }]),
        ]
      : [page("gameplay", [{ title: "sectionSlide", rows: [rows.type] }]), page("text", [{ title: "sectionText", rows: [...textRows(slide, rows), rows.notes] }])];
  // Ranking screens show the ranking, not a picture or video.
  const pictureVideo: Section[] = showsRanking(slide.type)
    ? []
    : [{ title: "sectionPictureVideo", rows: [rows.picture, ...videoRows(rows)] }];
  pages.push(
    page("design", [
      ...pictureVideo,
      { title: "sectionSound", rows: soundRows(rows) },
      { title: "sectionBackground", rows: [rows.background, ...rows.backgroundOptions] },
    ]),
    page("advanced", [{ title: "sectionMetadata", rows: [rows.id] }]),
  );
  return pages;
}

/** The video's rows: the file, how it plays, which part plays, its effect. */
function videoRows(rows: SlideProperties): PropertyRow[] {
  return [rows.video, ...rows.videoOptions, rows.videoPart, ...rows.videoReplay, rows.videoEffect].filter((row) => row !== null);
}

/** The sound's rows: the file, how it plays, which part plays, its loudness. */
function soundRows(rows: SlideProperties): PropertyRow[] {
  return [rows.sound, ...rows.soundOptions, rows.soundPart, ...rows.replay, rows.gain, rows.normalize].filter((row) => row !== null);
}

function textRows(slide: Exclude<Slide, { type: "question" }>, rows: SlideProperties): PropertyRow[] {
  switch (slide.type) {
    case "billboard":
      return [rows.title(slide), rows.bodyText(slide)];
    case "explanation":
      return [rows.explains, rows.explanationText(slide)];
    case "endOfRound":
    case "final":
      return [rows.title(slide)];
  }
}
