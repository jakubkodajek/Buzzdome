import { describe, expect, it } from "vitest";
import { MAX_ANSWERS } from "../bindings/Constants";
import type { QuestionSlide, Quiz } from "../types";
import {
  addAnswer,
  convertSlide,
  freshSlideId,
  newSlide,
  removeAnswer,
  setAnswerAt,
  setCorrect,
  setMode,
  setPicture,
  setScoring,
  setSound,
  setSoundOptions,
  setVideo,
  withAnswerCount,
} from "./slides";

const question = (patch: Partial<QuestionSlide> = {}): QuestionSlide => ({ ...(newSlide("question", "q") as QuestionSlide), ...patch });

describe("slides", () => {
  it("starts a question that can be played: four answers, the first one right", () => {
    const q = question();
    expect(q.answers).toEqual(["", "", "", ""]);
    expect(q.correct).toBe(0);
    expect([q.timeLimitS, q.points, q.penalty]).toEqual([20, 10, 0]);
  });

  it("gives everyone-answers questions answers and a right one", () => {
    const verbal = question({ mode: "fastestFinger", answers: [], correct: null });
    const everyone = setMode(verbal, "everyone");
    expect(everyone.answers).toHaveLength(4);
    expect(everyone.correct).toBe(0);
    // Answers already there are kept.
    const kept = setMode(question({ mode: "fastestFinger", answers: ["a", "b", "c"], correct: 2 }), "everyone");
    expect([kept.answers, kept.correct]).toEqual([["a", "b", "c"], 2]);
  });

  it("keeps the right answer when another answer is removed", () => {
    const q = question({ answers: ["a", "b", "c", "d"], correct: 2 });
    expect(removeAnswer(q, 0)).toMatchObject({ answers: ["b", "c", "d"], correct: 1 });
    expect(removeAnswer(q, 3)).toMatchObject({ answers: ["a", "b", "c"], correct: 2 });
    // Removing the right answer falls back to the first (everyone) or none (fastest finger).
    expect(removeAnswer(q, 2).correct).toBe(0);
    expect(removeAnswer({ ...q, mode: "fastestFinger" }, 2).correct).toBeNull();
  });

  it("never goes below two answers for everyone, or above the maximum", () => {
    const two = question({ answers: ["a", "b"], correct: 1 });
    expect(removeAnswer(two, 0)).toBe(two);
    const verbal = question({ mode: "fastestFinger", answers: ["a"], correct: null });
    expect(removeAnswer(verbal, 0).answers).toEqual([]);
    let full = question();
    for (let i = 0; i < 10; i++) full = addAnswer(full);
    expect(full.answers).toHaveLength(MAX_ANSWERS);
  });

  it("adds an answer typed into the row after the last one, and only there", () => {
    const q = question({ answers: ["a", "b"], correct: 1 });
    expect(setAnswerAt(q, 1, "B").answers).toEqual(["a", "B"]);
    expect(setAnswerAt(q, 2, "c").answers).toEqual(["a", "b", "c"]);
    expect(setAnswerAt(q, 3, "d")).toBe(q);
    const full = question({ answers: ["a", "b", "c", "d"] });
    expect(setAnswerAt(full, MAX_ANSWERS, "e")).toBe(full);
  });

  it("sets the number of answers as far as the mode allows", () => {
    const q = question({ answers: ["a", "b", "c", "d"], correct: 3 });
    expect(withAnswerCount(question({ answers: ["a", "b"] }), 4).answers).toEqual(["a", "b", "", ""]);
    expect(withAnswerCount(q, MAX_ANSWERS + 2)).toBe(q);
    expect(withAnswerCount(q, 2)).toMatchObject({ answers: ["a", "b"], correct: 0 });
    expect(withAnswerCount(q, 0).answers).toHaveLength(2);
    expect(withAnswerCount(q, 4)).toBe(q);
    expect(withAnswerCount({ ...q, mode: "fastestFinger" }, 0)).toMatchObject({ answers: [], correct: null });
  });

  it("only allows no right answer for fastest finger", () => {
    expect(setCorrect(question(), null).correct).toBe(0);
    expect(setCorrect(question({ mode: "fastestFinger" }), null).correct).toBeNull();
    expect(setCorrect(question(), 7).correct).toBe(0);
  });

  it("keeps the clock and points whole and in range", () => {
    const q = setScoring(question(), { timeLimitS: -5, points: 12.6, pointsEnd: 1e9, penalty: Number.NaN });
    expect([q.timeLimitS, q.points, q.pointsEnd, q.penalty]).toEqual([0, 13, 100_000, 0]);
  });

  it("shows a picture or a video, not both", () => {
    const withVideo = setVideo(question(), "media/clip.mp4");
    expect(withVideo.video?.autoplay).toBe(true);
    const withPicture = setPicture(withVideo, "media/flag.png");
    expect([withPicture.picture, withPicture.video]).toEqual(["media/flag.png", null]);
    expect(setVideo(withPicture, "media/clip.mp4").picture).toBeNull();
    // Taking the picture away leaves nothing else changed.
    expect(setPicture(withPicture, null)).toMatchObject({ picture: null, video: null });
  });

  it("keeps sound options when the file changes and the volume in percent", () => {
    const loud = setSoundOptions(setSound(question(), "media/a.mp3"), { start: "withClock", volume: 150 });
    expect(loud.sound).toEqual({ file: "media/a.mp3", start: "withClock", repeat: false, volume: 100 });
    expect(setSound(loud, "media/b.mp3").sound).toMatchObject({ file: "media/b.mp3", start: "withClock" });
  });

  it("converts a slide to another type, keeping its text, notes and media", () => {
    const q = { ...question({ text: "Who?", notes: "n" }), picture: "media/a.png" };
    const billboard = convertSlide(q, "billboard");
    expect(billboard).toMatchObject({ id: "q", type: "billboard", title: "Who?", text: "", notes: "n", picture: "media/a.png" });
    const back = convertSlide(billboard, "question");
    expect(back).toMatchObject({ type: "question", text: "Who?", answers: ["", "", "", ""], correct: 0 });
    expect(convertSlide(q, "question")).toBe(q);
  });

  it("makes ids no slide has", () => {
    const quiz: Quiz = { schemaVersion: 1, title: "", theme: null, slides: [newSlide("billboard", "s-a")] };
    const ids = ["a", "a", "b"];
    expect(freshSlideId(quiz, () => ids.shift() as string)).toBe("s-b");
  });
});
