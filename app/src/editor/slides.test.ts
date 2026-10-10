import { describe, expect, it } from "vitest";
import { MAX_ANSWERS, MAX_EVERYONE_ANSWERS } from "../bindings/Constants";
import type { Category, QuestionSlide, Quiz } from "../types";
import {
  addAnswer,
  convertSlide,
  forgetCategory,
  formatImported,
  formatLike,
  freshSlideId,
  fullyCorrect,
  headlinePart,
  newSlide,
  removeAnswer,
  setAnswerAt,
  setBackground,
  setBackgroundOptions,
  setBodyText,
  setCorrect,
  setCorrectAnswers,
  setHeadline,
  setMode,
  setNotes,
  setPicture,
  setScoring,
  setShare,
  setSlideFill,
  setSound,
  setSoundOptions,
  setVideo,
  setVideoOptions,
  setTextStyle,
  styledParts,
  toggleCorrect,
  withAnswerCount,
  withFreshIds,
} from "./slides";

const question = (patch: Partial<QuestionSlide> = {}): QuestionSlide => ({ ...(newSlide("question", "q") as QuestionSlide), ...patch });
/** Answers `answers` as right with all of the points. */
const right = (...answers: number[]) => answers.map(fullyCorrect);
/** The numbers of the question's right answers. */
const rightOf = (q: QuestionSlide) => q.correct.map((c) => c.answer);

describe("slides", () => {
  it("starts a question that can be played: four answers, the first one right", () => {
    const q = question();
    expect(q.answers).toEqual(["", "", "", ""]);
    expect(q.correct).toEqual([{ answer: 0, share: 100 }]);
    expect([q.timeLimitS, q.points, q.penalty]).toEqual([20, 10, 0]);
  });

  it("gives everyone-answers questions answers and a right one", () => {
    const verbal = question({ mode: "fastestFinger", answers: [], correct: [] });
    const everyone = setMode(verbal, "everyone");
    expect(everyone.answers).toHaveLength(4);
    expect(rightOf(everyone)).toEqual([0]);
    // Answers already there are kept.
    const kept = setMode(question({ mode: "fastestFinger", answers: ["a", "b", "c"], correct: right(2) }), "everyone");
    expect([kept.answers, rightOf(kept)]).toEqual([["a", "b", "c"], [2]]);
    // Answers past the buttons go; right ones among them go too, and with none left the first is right.
    const six = question({ mode: "fastestFinger", answers: ["a", "b", "c", "d", "e", "f"], correct: right(1, 4) });
    expect(rightOf(setMode(six, "everyone"))).toEqual([1]);
    expect(setMode({ ...six, correct: right(4) }, "everyone")).toMatchObject({ answers: ["a", "b", "c", "d"], correct: right(0) });
  });

  it("gives every right answer all of the points on a fastest finger question", () => {
    const shared = question({ answers: ["a", "b", "c"], correct: [fullyCorrect(0), { answer: 2, share: 50 }] });
    expect(setMode(shared, "fastestFinger").correct).toEqual(right(0, 2));
  });

  it("keeps the right answers when another answer is removed", () => {
    const q = question({ answers: ["a", "b", "c", "d"], correct: [{ answer: 1, share: 50 }, fullyCorrect(2)] });
    expect(removeAnswer(q, 0)).toMatchObject({ answers: ["b", "c", "d"], correct: [{ answer: 0, share: 50 }, fullyCorrect(1)] });
    expect(removeAnswer(q, 3)).toMatchObject({ answers: ["a", "b", "c"], correct: q.correct });
    expect(removeAnswer(q, 1).correct).toEqual(right(1));
    // Removing the last right answer falls back to the first (everyone) or none (fastest finger).
    const one = question({ answers: ["a", "b", "c", "d"], correct: right(2) });
    expect(removeAnswer(one, 2).correct).toEqual(right(0));
    expect(removeAnswer({ ...one, mode: "fastestFinger" }, 2).correct).toEqual([]);
  });

  it("never goes below two answers for everyone, or above the maximum", () => {
    const two = question({ answers: ["a", "b"], correct: right(1) });
    expect(removeAnswer(two, 0)).toBe(two);
    const verbal = question({ mode: "fastestFinger", answers: ["a"], correct: [] });
    expect(removeAnswer(verbal, 0).answers).toEqual([]);
    let full = question();
    for (let i = 0; i < 10; i++) full = addAnswer(full);
    expect(full.answers).toHaveLength(MAX_EVERYONE_ANSWERS);
    // Fastest finger is answered aloud, so it may show every answer letter.
    let aloud = question({ mode: "fastestFinger" });
    for (let i = 0; i < 10; i++) aloud = addAnswer(aloud);
    expect(aloud.answers).toHaveLength(MAX_ANSWERS);
  });

  it("adds an answer typed into the row after the last one, and only there", () => {
    const q = question({ answers: ["a", "b"], correct: right(1) });
    expect(setAnswerAt(q, 1, "B").answers).toEqual(["a", "B"]);
    expect(setAnswerAt(q, 2, "c").answers).toEqual(["a", "b", "c"]);
    expect(setAnswerAt(q, 3, "d")).toBe(q);
    const full = question({ answers: ["a", "b", "c", "d"] });
    expect(setAnswerAt(full, MAX_EVERYONE_ANSWERS, "e")).toBe(full);
    expect(setAnswerAt({ ...full, mode: "fastestFinger" }, MAX_EVERYONE_ANSWERS, "e").answers).toHaveLength(MAX_EVERYONE_ANSWERS + 1);
  });

  it("sets the number of answers as far as the mode allows", () => {
    const q = question({ answers: ["a", "b", "c", "d"], correct: right(3) });
    expect(withAnswerCount(question({ answers: ["a", "b"] }), 4).answers).toEqual(["a", "b", "", ""]);
    expect(withAnswerCount(q, MAX_ANSWERS + 2)).toBe(q);
    expect(withAnswerCount(q, 2)).toMatchObject({ answers: ["a", "b"], correct: right(0) });
    expect(withAnswerCount(q, 0).answers).toHaveLength(2);
    expect(withAnswerCount(q, 4)).toBe(q);
    expect(withAnswerCount({ ...q, mode: "fastestFinger" }, 0)).toMatchObject({ answers: [], correct: [] });
  });

  it("marks several answers right, in order, each with all of the points", () => {
    const q = setCorrect(setCorrect(question(), 3, true), 2, true);
    expect(q.correct).toEqual(right(0, 2, 3));
    expect(setCorrect(q, 0, false).correct).toEqual(right(2, 3));
    // Already so, or no such answer: the same question.
    expect(setCorrect(q, 2, true)).toBe(q);
    expect(setCorrect(q, 1, false)).toBe(q);
    expect(setCorrect(q, 7, true)).toBe(q);
    expect(toggleCorrect(q, 1).correct).toEqual(right(0, 1, 2, 3));
    expect(toggleCorrect(q, 3).correct).toEqual(right(0, 2));
  });

  it("only allows no right answer for fastest finger", () => {
    const q = question();
    expect(setCorrect(q, 0, false)).toBe(q);
    expect(toggleCorrect(q, 0)).toBe(q);
    expect(setCorrectAnswers(q, [])).toBe(q);
    expect(setCorrect({ ...q, mode: "fastestFinger" }, 0, false).correct).toEqual([]);
  });

  it("sets the right answers at once, keeping the shares of those that stay right", () => {
    const q = question({ correct: [{ answer: 1, share: 50 }] });
    expect(setCorrectAnswers(q, [3, 1, 3, 9, -1, 0.5]).correct).toEqual([{ answer: 1, share: 50 }, fullyCorrect(3)]);
    expect(setCorrectAnswers(q, [9]).correct).toEqual(q.correct);
  });

  it("keeps a right answer's share a whole percent from 1 to 100", () => {
    const q = question({ correct: right(0, 1) });
    expect(setShare(q, 1, 49.6).correct).toEqual([fullyCorrect(0), { answer: 1, share: 50 }]);
    expect(setShare(q, 1, 0).correct[1].share).toBe(1);
    expect(setShare(q, 1, 250).correct[1].share).toBe(100);
    // A wrong answer has no share to set, nor has a fastest finger one.
    expect(setShare(q, 2, 50)).toBe(q);
    const fastest = setMode(q, "fastestFinger");
    expect(setShare(fastest, 1, 50)).toBe(fastest);
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
    expect(loud.sound).toEqual({
      file: "media/a.mp3",
      start: "withClock",
      repeat: false,
      volume: 100,
      part: { fromMs: 0, toMs: null },
      gainDb: 0,
      replay: null,
    });
    expect(setSound(loud, "media/b.mp3").sound).toMatchObject({ file: "media/b.mp3", start: "withClock" });
  });

  it("keeps a sound's parts playable and its gain within limits", () => {
    const song = setSound(question(), "media/song.mp3");
    const chorus = setSoundOptions(song, { part: { fromMs: 15_000.4, toMs: 45_000 }, gainDb: 3.14159 });
    expect(chorus.sound).toMatchObject({ part: { fromMs: 15_000, toMs: 45_000 }, gainDb: 3.1 });
    // A part that holds no sound is refused; the gain stops at its limits.
    expect(setSoundOptions(chorus, { part: { fromMs: 9_000, toMs: 9_000 } })).toBe(chorus);
    expect(setSoundOptions(chorus, { gainDb: 99 }).sound?.gainDb).toBe(20);
    expect(setSoundOptions(chorus, { gainDb: -99 }).sound?.gainDb).toBe(-30);
    const replay = { part: { fromMs: 60_000, toMs: null }, delayMs: 999_999 };
    expect(setSoundOptions(chorus, { replay }).sound?.replay).toEqual({ part: { fromMs: 60_000, toMs: null }, delayMs: 60_000 });
    expect(setSoundOptions(chorus, { replay: { part: { fromMs: 5, toMs: 1 }, delayMs: 0 } })).toBe(chorus);
  });

  it("gives only questions a replay part, since only they reveal an answer", () => {
    const replay = { part: { fromMs: 0, toMs: 10_000 }, delayMs: 1_000 };
    const music = setSoundOptions(setSound(question(), "media/song.mp3"), { replay });
    expect(music.sound?.replay).toEqual(replay);
    const billboard = convertSlide(music, "billboard");
    expect(billboard.sound).toMatchObject({ file: "media/song.mp3", replay: null });
    expect(setSoundOptions(billboard, { replay }).sound?.replay).toBeNull();
  });

  it("keeps a video's options playable: parts, volume, speed and intro pause within limits", () => {
    const clip = setVideo(question(), "media/clip.mp4");
    expect(clip.video).toEqual({
      file: "media/clip.mp4",
      autoplay: true,
      introPauseMs: 1_000,
      muted: false,
      repeat: false,
      volume: 100,
      speed: 100,
      part: { fromMs: 0, toMs: null },
      effect: null,
      replay: null,
    });
    const set = setVideoOptions(clip, { part: { fromMs: 5_000.4, toMs: 20_000 }, volume: 140, speed: 999, introPauseMs: 120_000 });
    expect(set.video).toMatchObject({ part: { fromMs: 5_000, toMs: 20_000 }, volume: 100, speed: 250, introPauseMs: 60_000 });
    expect(setVideoOptions(set, { speed: 5 }).video?.speed).toBe(25);
    // An emptied field counts as 0, brought into range.
    expect(setVideoOptions(set, { speed: Number.NaN, introPauseMs: Number.NaN }).video).toMatchObject({ speed: 25, introPauseMs: 0 });
    expect(setVideoOptions(set, { part: { fromMs: 9_000, toMs: 3_000 } })).toBe(set);
    // Replacing the file keeps the options.
    expect(setVideo(set, "media/other.webm").video).toMatchObject({ file: "media/other.webm", speed: 250 });
  });

  it("gives only questions a video effect and a video replay part", () => {
    const replay = { part: { fromMs: 10_000, toMs: null }, delayMs: 500 };
    const clip = setVideoOptions(setVideo(question(), "media/clip.mp4"), { effect: "mosaic", replay });
    expect(clip.video).toMatchObject({ effect: "mosaic", replay });
    const billboard = convertSlide(clip, "billboard");
    expect(billboard.video).toMatchObject({ file: "media/clip.mp4", effect: null, replay: null });
    expect(setVideoOptions(billboard, { effect: "blur", replay }).video).toMatchObject({ effect: null, replay: null });
  });

  it("keeps background options when the file changes", () => {
    const loop = setBackground(question(), "media/stage.mp4");
    expect(loop.background).toEqual({ file: "media/stage.mp4", repeat: true, muted: true, sync: false });
    const synced = setBackground(setBackgroundOptions(loop, { sync: true, muted: false }), "media/other.mp4");
    expect(synced.background).toEqual({ file: "media/other.mp4", repeat: true, muted: false, sync: true });
    expect(setBackground(synced, null).background).toBeNull();
    expect(setBackgroundOptions(question(), { sync: true })).toEqual(question());
  });

  it("converts a slide to another type, keeping its text, notes and media", () => {
    const q = { ...question({ text: "Who?", notes: "n" }), picture: "media/a.png" };
    const billboard = convertSlide(q, "billboard");
    expect(billboard).toMatchObject({ id: "q", type: "billboard", title: "Who?", text: "", notes: "n", picture: "media/a.png" });
    const back = convertSlide(billboard, "question");
    expect(back).toMatchObject({ type: "question", text: "Who?", answers: ["", "", "", ""], correct: right(0) });
    expect(convertSlide(q, "question")).toBe(q);
  });

  it("keeps only the style that is set, and the style through a change of type", () => {
    const fill = { from: "#000000", to: "#FFFFFF", kind: "vertical" } as const;
    const styled = setTextStyle(setSlideFill(question(), fill), "headline", { color: "#FF0000", bold: false });
    expect(styled.style).toEqual({ fill, headline: { color: "#FF0000", bold: false } });
    expect(setTextStyle(styled, "headline", { bold: undefined }).style).toEqual({ fill, headline: { color: "#FF0000" } });
    expect(convertSlide(styled, "billboard").style).toEqual(styled.style);
    // Back to the theme's look: no style at all.
    const plain = setSlideFill(setTextStyle(styled, "headline", { color: undefined, bold: undefined }), null);
    expect(plain.style).toBeUndefined();
    expect(JSON.stringify(plain)).toBe(JSON.stringify(question()));
  });

  it("keeps the headline's look with the headline when the type changes, and starts the other texts anew", () => {
    const fill = { from: "#000000", to: "#FFFFFF", kind: "vertical" } as const;
    const headline = { color: "#FF0000" };
    const styled = setTextStyle(setTextStyle(setSlideFill(question({ text: "Why?" }), fill), "headline", headline), "answers", { bold: true });
    // An explanation's text is its headline, so the question's look goes there; it has no answers.
    const explanation = convertSlide(styled, "explanation");
    expect(explanation).toMatchObject({ type: "explanation", text: "Why?" });
    expect(explanation.style).toEqual({ fill, text: headline });
    expect(styledParts(explanation)).toContain(headlinePart("explanation"));
    // And back from its text to a billboard's title; the billboard's own text starts as the theme has it.
    expect(convertSlide(explanation, "billboard").style).toEqual({ fill, headline });
    const billboard = setTextStyle(convertSlide(explanation, "billboard"), "text", { italic: true });
    expect(convertSlide(billboard, "endOfRound").style).toEqual({ fill, headline });
    // A slide without a look of its own stays without one.
    expect(convertSlide(question(), "explanation").style).toBeUndefined();
  });

  it("styles the headline of every type of slide as one of its parts", () => {
    for (const type of ["billboard", "question", "explanation", "endOfRound", "final"] as const) {
      expect(styledParts(newSlide(type, "s"))[0]).toBe(headlinePart(type));
    }
  });

  it("plays a sound that went with the clock when the slide opens once the slide has no clock", () => {
    const timed = setSoundOptions(setSound(question(), "media/tick.mp3"), { start: "withClock" });
    expect(convertSlide(timed, "billboard").sound?.start).toBe("onSlide");
    const onDemand = setSoundOptions(timed, { start: "onDemand" });
    expect(convertSlide(onDemand, "explanation").sound?.start).toBe("onDemand");
  });

  it("drops the picture or video of a slide that becomes a ranking, and keeps its sound", () => {
    const pictured = setSound({ ...question(), picture: "media/a.png" }, "media/song.mp3");
    const filmed = setVideo(question(), "media/clip.mp4");
    for (const type of ["final", "endOfRound"] as const) {
      expect(convertSlide(pictured, type)).toMatchObject({ picture: null, video: null, sound: { file: "media/song.mp3" } });
      expect(convertSlide(filmed, type)).toMatchObject({ picture: null, video: null });
    }
  });

  it("sets each slide's texts where that slide has them", () => {
    expect(setHeadline(question(), "Who?")).toMatchObject({ text: "Who?" });
    expect(setHeadline(newSlide("explanation", "e"), "Because")).toMatchObject({ text: "Because" });
    expect(setHeadline(newSlide("endOfRound", "r"), "End")).toMatchObject({ title: "End" });
    expect(setHeadline(newSlide("final", "f"), "Winners")).toMatchObject({ type: "final", title: "Winners" });
    const billboard = setBodyText(setHeadline(newSlide("billboard", "b"), "Round 1"), "Geography");
    expect(billboard).toMatchObject({ title: "Round 1", text: "Geography" });
    // Only a billboard has text under its title.
    const end = newSlide("endOfRound", "r");
    expect(setBodyText(end, "x")).toBe(end);
    expect(setNotes(end, "Applause")).toMatchObject({ notes: "Applause" });
  });

  it("makes ids no slide has", () => {
    const quiz: Quiz = { schemaVersion: 1, title: "", theme: null, slides: [newSlide("billboard", "s-a")] };
    const ids = ["a", "a", "b"];
    expect(freshSlideId(quiz, () => ids.shift() as string)).toBe("s-b");
  });

  it("gives slides to insert ids no slide has, nor each other", () => {
    const quiz: Quiz = { schemaVersion: 1, title: "", theme: null, slides: [newSlide("billboard", "s-a")] };
    const ids = ["a", "b", "b", "c"];
    const imported = [newSlide("billboard", "import-1"), newSlide("explanation", "import-2")];
    const fresh = withFreshIds(quiz, imported, () => ids.shift() as string);
    expect(fresh.map((slide) => slide.id)).toEqual(["s-b", "s-c"]);
    expect(fresh[1]).toEqual({ ...imported[1], id: "s-c" });
  });

  it("keeps a category on copies to insert only while the quiz has it", () => {
    const quiz: Quiz = { schemaVersion: 1, title: "", theme: null, slides: [], categories: [{ id: "c-1", name: "Films", background: null }] };
    const copied = [{ ...newSlide("question", "a"), category: "c-1" }, { ...newSlide("question", "b"), category: "c-gone" }];
    expect(withFreshIds(quiz, copied).map((slide) => slide.category)).toEqual(["c-1", undefined]);
  });
});

describe("categories on slides", () => {
  const red = "#c80000";
  const films: Category = {
    id: "c-1",
    name: "Films",
    background: { file: "media/loop.mp4", repeat: true, muted: true, sync: false },
    style: { fill: { from: red, to: red, kind: "vertical" }, headline: { bold: true }, text: { italic: true }, answers: { color: red } },
  };

  it("formats a slide like a category: its fill, the looks of the texts the slide shows and its background", () => {
    const own = setPicture(setTextStyle(question(), "headline", { size: 80 }), "media/actor.jpg");
    const formatted = formatLike(own, films);
    expect(formatted.style).toEqual({ fill: films.style?.fill, headline: { bold: true }, answers: { color: red } });
    expect(formatted).toMatchObject({ background: films.background, category: "c-1", picture: "media/actor.jpg" });
    // A billboard shows a text and no answers.
    expect(formatLike(newSlide("billboard", "b"), films).style).toEqual({ fill: films.style?.fill, headline: { bold: true }, text: { italic: true } });
  });

  it("keeps the slide's own background for a category without one, and an imported row's own", () => {
    const plain: Category = { id: "c-2", name: "Music", background: null };
    const withBackground = setBackground(question(), "media/stage.jpg");
    expect(formatLike(withBackground, plain)).toMatchObject({ background: withBackground.background, category: "c-2" });
    expect(formatLike(withBackground, plain).style).toBeUndefined();
    const imported = { ...withBackground, category: "c-1" };
    expect(formatImported(imported, [films])).toMatchObject({ background: withBackground.background, style: { headline: { bold: true } } });
    expect(formatImported({ ...question(), category: "c-1" }, [films]).background).toBe(films.background);
    // A row without a category, or naming one gone, stays as imported.
    expect(formatImported(withBackground, [films])).toBe(withBackground);
    expect(formatImported({ ...withBackground, category: "c-9" }, [films])).toEqual({ ...withBackground, category: "c-9" });
  });

  it("forgets a category and keeps its look, and keeps it through a change of type", () => {
    const formatted = formatLike(question(), films);
    expect(forgetCategory(formatted)).toEqual({ ...formatted, category: undefined });
    expect(forgetCategory(question())).toEqual(question());
    expect(convertSlide(formatted, "billboard").category).toBe("c-1");
  });
});
