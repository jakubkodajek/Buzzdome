import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import { NO_CONTEXT } from "../quiz";
import type { Category, QuestionSlide, Slide, SlideType } from "../types";
import { sampleSlide } from "./categories";
import type { ElementId } from "./elements";
import type { SlideEditing } from "./properties";
import { categoryPages, selectionPages, type Page } from "./property-pages";
import { newSlide } from "./slides";

const t = translator("en");

function editing(slide: Slide, categories: Category[] = []): SlideEditing {
  return { slide, context: NO_CONTEXT, categories, update: () => {}, chooseMedia: () => {}, trimSound: () => {}, trimVideo: () => {}, normalizeSound: () => {} };
}

/** Pages as page ids, section titles and row ids. */
const outlined = (pages: Page[]) => pages.map((p) => ({ [p.id]: p.sections.map((s) => ({ [s.title]: s.rows.map((r) => r.id) })) }));

/** The pages for the selected slides of a quiz with `categories`, outlined; the mixed row ids. */
function outline(slides: Slide[], element: ElementId = "slide", categories: Category[] = []) {
  const { pages, mixed } = selectionPages(t, slides.map((s) => editing(s, categories)), element);
  return { pages: outlined(pages), mixed: [...mixed].sort() };
}

/** The ids of the shown pages. */
const pageIds = (slides: Slide[], element?: ElementId) => outline(slides, element).pages.map((p) => Object.keys(p)[0]);

/** Every shown row id, in order. */
const rowIds = (slides: Slide[], element?: ElementId) => outline(slides, element).pages.flatMap((p) => Object.values(p)[0].flatMap((s) => Object.values(s)[0]));

const question = (patch: Partial<QuestionSlide> = {}): QuestionSlide => ({ ...(newSlide("question", "q") as QuestionSlide), ...patch });
const slide = (type: SlideType) => newSlide(type, type);
const look = (part: string) => ["font", "color", "align", "shadow", "shadowColor", "fill", "transparency"].map((row) => `${part}.${row}`);

describe("the pages for one slide", () => {
  it("are a question's pages, the most used first", () => {
    expect(outline([question()])).toEqual({
      pages: [
        { gameplay: [{ sectionSlide: ["type"] }, { sectionTiming: ["answerTime"] }, { sectionAnswering: ["everyoneAnswers", "answerCount", "correctAnswer"] }] },
        { scoring: [{ sectionPoints: ["pointsStart", "falling", "penalty"] }] },
        { text: [{ sectionText: ["questionText", "notes"] }] },
        { media: [{ sectionPictureVideo: ["picture", "video"] }, { sectionSound: ["sound"] }] },
        { design: [{ sectionBackground: ["fill", "background"] }, { sectionQuestionLook: look("headline") }, { sectionAnswersLook: look("answers") }] },
        { advanced: [{ sectionMetadata: ["id"] }] },
      ],
      mixed: [],
    });
  });

  it("give falling points their end", () => {
    expect(rowIds([question({ pointsEnd: 2 })])).toContain("pointsEnd");
    expect(rowIds([question()])).not.toContain("pointsEnd");
  });

  it("have a look section for each text the slide type shows", () => {
    const looks = (type: SlideType) => outline([slide(type)]).pages.find((p) => "design" in p)?.design.map((s) => Object.keys(s)[0]);
    expect(looks("billboard")).toEqual(["sectionBackground", "sectionTitleLook", "sectionTextLook"]);
    expect(looks("explanation")).toEqual(["sectionBackground", "sectionTextLook"]);
    expect(looks("final")).toEqual(["sectionBackground", "sectionTitleLook"]);
  });

  it("offer a picture or video only where the slide type plays one", () => {
    expect(rowIds([slide("billboard")])).toEqual(expect.arrayContaining(["picture", "video", "sound"]));
    for (const type of ["endOfRound", "final"] as const) {
      expect(rowIds([slide(type)])).toContain("sound");
      expect(rowIds([slide(type)])).not.toContain("picture");
      expect(rowIds([slide(type)])).not.toContain("video");
    }
  });

  it("are none for no slide", () => {
    expect(selectionPages(t, [], "slide")).toEqual({ pages: [], mixed: new Set() });
  });
});

describe("the pages for a part of a slide", () => {
  it("are an answer's rows and its look, with a share only for a right answer", () => {
    expect(outline([question()], "answer-1").pages).toEqual([
      { text: [{ sectionAnswer: ["answer1.text", "answer1.correct", "answer1.remove"] }] },
      { design: [{ sectionAnswersLook: look("answers") }] },
    ]);
    expect(rowIds([question()], "answer-0")).toEqual(["answer0.text", "answer0.correct", "answer0.share", "answer0.remove", ...look("answers")]);
  });

  it("are the question's text and its look", () => {
    expect(outline([question()], "question").pages).toEqual([{ text: [{ sectionText: ["questionText"] }] }, { design: [{ sectionQuestionLook: look("headline") }] }]);
  });

  it("are a text's rows, with a look only for a text the slide type shows", () => {
    expect(outline([slide("billboard")], "text").pages).toEqual([{ text: [{ sectionText: ["title", "bodyText"] }] }, { design: [{ sectionTextLook: look("text") }] }]);
    expect(outline([slide("explanation")], "text").pages).toEqual([{ text: [{ sectionText: ["explains", "explanationText"] }] }, { design: [{ sectionTextLook: look("text") }] }]);
    // An explanation has no headline to give a look.
    expect(pageIds([slide("explanation")], "title")).toEqual(["text"]);
  });

  it("are a medium's rows on the media page alone", () => {
    expect(outline([question()], "picture").pages).toEqual([{ media: [{ sectionPictureVideo: ["picture"] }] }]);
    expect(outline([question()], "video").pages).toEqual([{ media: [{ sectionPictureVideo: ["video"] }] }]);
    expect(outline([question()], "sound").pages).toEqual([{ media: [{ sectionSound: ["sound"] }] }]);
  });
});

describe("the pages for several slides", () => {
  it("show the rows every slide has with a value, the ones they differ in mixed", () => {
    const { pages, mixed } = outline([question(), question({ timeLimitS: 30, penalty: 5 })]);
    expect(mixed).toEqual(["answerTime", "penalty"]);
    // Texts and the id are each slide's own, so they and their pages go.
    expect(pages.map((p) => Object.keys(p)[0])).toEqual(["gameplay", "scoring", "media", "design"]);
    expect(rowIds([question(), question({ timeLimitS: 30 })])).not.toContain("questionText");
  });

  it("show the slide's own pages whatever part of the first was selected", () => {
    expect(pageIds([question(), question()], "answer-1")).toEqual(pageIds([question(), question()]));
  });

  it("show a row only some slides have nowhere", () => {
    expect(rowIds([question({ pointsEnd: 2 }), question()])).not.toContain("pointsEnd");
    expect(outline([question({ pointsEnd: 2 }), question({ pointsEnd: 4 })]).mixed).toContain("pointsEnd");
  });

  it("of other types share only what both have, the type mixed", () => {
    const { pages, mixed } = outline([question(), slide("billboard")]);
    expect(mixed).toEqual(["type"]);
    expect(pages).toEqual([
      { gameplay: [{ sectionSlide: ["type"] }] },
      { media: [{ sectionPictureVideo: ["picture", "video"] }, { sectionSound: ["sound"] }] },
      { design: [{ sectionBackground: ["fill", "background"] }, { sectionQuestionLook: look("headline") }] },
    ]);
    // A final slide plays no picture, so neither does the selection.
    expect(rowIds([slide("billboard"), slide("final")])).not.toContain("picture");
  });
});

describe("categories in the pages", () => {
  const films: Category = { id: "c-1", name: "Films", background: null };

  it("offer a slide of a quiz with categories the one it takes on, first on the design page", () => {
    const design = (slides: Slide[], categories: Category[]) => outline(slides, "slide", categories).pages.find((p) => "design" in p)?.design;
    expect(design([question()], [films])?.[0]).toEqual({ sectionCategory: ["category"] });
    expect(design([question()], [])?.[0]).toEqual({ sectionBackground: ["fill", "background"] });
    expect(outline([question({ category: "c-1" }), question()], "slide", [films]).mixed).toEqual(["category"]);
  });

  it("are a category's name, its background and the look of every text, or of the text selected on its sample", () => {
    const name = { id: "category.name", label: "Name", description: "", control: null };
    const pages = (element: ElementId) => outlined(categoryPages(t, editing(sampleSlide(films)), name, element));
    expect(pages("slide")).toEqual([
      {
        design: [
          { sectionCategory: ["category.name"] },
          { sectionBackground: ["fill", "background"] },
          { sectionHeadlineLook: look("headline") },
          { sectionTextLook: look("text") },
          { sectionAnswersLook: look("answers") },
        ],
      },
    ]);
    expect(pages("answer-2")).toEqual([{ design: [{ sectionAnswersLook: look("answers") }] }]);
  });
});
