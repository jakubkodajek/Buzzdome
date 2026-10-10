import { describe, expect, it } from "vitest";
import type { QuestionSlide } from "../types";
import { answerIndex, hasElement, isMediaElement, type ElementId } from "./elements";
import { newSlide, setPicture } from "./slides";

const question = newSlide("question", "q") as QuestionSlide;
const billboard = newSlide("billboard", "b");

describe("the parts of a slide", () => {
  it("tell a media file from a text, an answer and the clock", () => {
    const media: ElementId[] = ["picture", "video", "sound"];
    const others: ElementId[] = ["slide", "question", "title", "text", "timer", "answer-0"];
    expect(media.filter(isMediaElement)).toEqual(media);
    expect(others.filter(isMediaElement)).toEqual([]);
  });

  it("number the answers", () => {
    expect(answerIndex("answer-3")).toBe(3);
    expect(answerIndex("question")).toBeNull();
  });

  it("are the slide's only while it has them", () => {
    expect(hasElement(question, "timer")).toBe(true);
    expect(hasElement(billboard, "timer")).toBe(false);
    expect(hasElement(question, `answer-${question.answers.length - 1}`)).toBe(true);
    expect(hasElement(question, `answer-${question.answers.length}`)).toBe(false);
    expect(hasElement(billboard, "text")).toBe(true);
    expect(hasElement(question, "picture")).toBe(false);
    expect(hasElement(setPicture(question, "media/a.jpg"), "picture")).toBe(true);
  });
});
