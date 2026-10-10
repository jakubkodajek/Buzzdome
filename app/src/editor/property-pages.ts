// The pages and sections the properties panel shows for the selection, and
// which rows several selected slides share: one place for the panel and
// the ribbon, which marks the same values as Mixed.

import type { StringKey, Translate } from "../i18n";
import { isCorrect, plays } from "../quiz";
import type { Slide } from "../types";
import { answerIndex, type ElementId } from "./elements";
import { slideProperties, type SlideEditing, type SlideProperties } from "./properties";
import { commonRows, type PropertyRow } from "./property-rows";
import { styledParts, type StyledPart } from "./slides";
import { lookTitle, styledPartOf, styleProperties, type StyleProperties } from "./style-properties";

export type PageId = "gameplay" | "scoring" | "text" | "media" | "design" | "advanced";

export interface Section {
  title: StringKey;
  rows: PropertyRow[];
}

export interface Page {
  id: PageId;
  sections: Section[];
}

/**
 * The pages for the selection: the first slide's (or the part of it
 * selected), and for several slides only the rows all of them have, with
 * the ids of those they differ in. A change through a row sets every
 * selected slide, as `editings` say.
 */
export function selectionPages(t: Translate, editings: SlideEditing[], element: ElementId): { pages: Page[]; mixed: ReadonlySet<string> } {
  const rows = editings.map((editing) => ({ slide: slideProperties(t, editing), style: styleProperties(t, editing) }));
  if (editings.length < 2) return { pages: editings.length === 0 ? [] : propertyPages(editings[0].slide, element, rows[0]), mixed: new Set() };
  const pages = editings.map((editing, i) => propertyPages(editing.slide, "slide", rows[i]));
  const { shown, mixed } = commonRows(pages.map(pagesRows));
  return { pages: withRows(pages[0], (row) => shown.has(row.id)), mixed };
}

/** Every row of the pages, in order. */
export function pagesRows(pages: Page[]): PropertyRow[] {
  return pages.flatMap((p) => p.sections.flatMap((s) => s.rows));
}

/** The pages with only the rows `keep` keeps; empty sections and pages go. */
export function withRows(pages: Page[], keep: (row: PropertyRow) => boolean): Page[] {
  return pages
    .map((p) => ({ ...p, sections: p.sections.map((s) => ({ ...s, rows: s.rows.filter(keep) })).filter((s) => s.rows.length > 0) }))
    .filter((p) => p.sections.length > 0);
}

/** The pages for the selected part of the slide, the most used first. */
function propertyPages(slide: Slide, element: ElementId, { slide: rows, style }: { slide: SlideProperties; style: StyleProperties }): Page[] {
  const page = (id: PageId, sections: Section[]): Page => ({ id, sections });
  const answer = answerIndex(element);
  const look = (part: StyledPart): Section => ({ title: lookTitle(slide, part), rows: style.look(part) });
  // A text selected on the canvas has its look on the design page.
  const part = styledPartOf(element);
  const lookPage = part && styledParts(slide).includes(part) ? [page("design", [look(part)])] : [];

  if (answer !== null && slide.type === "question") {
    return [
      page("text", [
        {
          title: "sectionAnswer",
          rows: [
            rows.answerText(slide, answer),
            rows.isCorrect(slide, answer),
            ...(isCorrect(slide, answer) ? [rows.share(slide, answer)] : []),
            rows.removeAnswer(slide, answer),
          ],
        },
      ]),
      ...lookPage,
    ];
  }
  if (element === "question" && slide.type === "question") return [page("text", [{ title: "sectionText", rows: [rows.questionText(slide)] }]), ...lookPage];
  if ((element === "title" || element === "text") && slide.type !== "question") return [page("text", [{ title: "sectionText", rows: textRows(slide, rows) }]), ...lookPage];
  if (element === "picture") return [page("media", [{ title: "sectionPictureVideo", rows: [rows.picture] }])];
  if (element === "video") return [page("media", [{ title: "sectionPictureVideo", rows: videoRows(rows) }])];
  if (element === "sound") return [page("media", [{ title: "sectionSound", rows: soundRows(rows) }])];

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
  const pictureVideo: Section[] = plays(slide.type).pictureAndVideo ? [{ title: "sectionPictureVideo", rows: [rows.picture, ...videoRows(rows)] }] : [];
  pages.push(
    page("media", [...pictureVideo, { title: "sectionSound", rows: soundRows(rows) }]),
    page("design", [{ title: "sectionBackground", rows: [style.fill, rows.background, ...rows.backgroundOptions] }, ...styledParts(slide).map(look)]),
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
