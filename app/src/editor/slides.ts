// Changes to one slide. Each keeps the slide playable: the editor offers no
// way to make a question the show would refuse (see `Quiz::validate` in
// crates/quiz-core/src/model.rs).

import { ANSWER_LETTERS } from "../answers";
import { slideTitle } from "../quiz";
import type { AnswerMode, Quiz, QuestionSlide, Slide, SlideType, SoundClip, VideoClip } from "../types";

/** Most answers a question can have (`MAX_ANSWERS` in Rust). */
export const MAX_ANSWERS = ANSWER_LETTERS.length;
/** Fewest answers an everyone-answers question can have. */
export const MIN_EVERYONE_ANSWERS = 2;
/** Answers a new everyone-answers question starts with: one per buzzer button. */
const DEFAULT_ANSWERS = 4;

/** A new, empty slide. */
export function newSlide(type: SlideType, id: string): Slide {
  const base = { id, notes: "", picture: null, video: null, sound: null, background: null };
  switch (type) {
    case "billboard":
      return { ...base, type, title: "", text: "" };
    case "question":
      // The same defaults as a quiz file that leaves the fields out.
      return {
        ...base,
        type,
        text: "",
        mode: "everyone",
        answers: Array<string>(DEFAULT_ANSWERS).fill(""),
        correct: 0,
        timeLimitS: 20,
        points: 10,
        pointsEnd: null,
        penalty: 0,
      };
    case "explanation":
      return { ...base, type, text: "" };
    case "endOfRound":
      return { ...base, type, title: "" };
  }
}

/** Sets the text the slide is listed by (see `slideTitle`). */
export function setHeadline(slide: Slide, text: string): Slide {
  return slide.type === "question" || slide.type === "explanation" ? { ...slide, text } : { ...slide, title: text };
}

/**
 * The slide as another type. Its id, notes and media stay, the question,
 * explanation or title carries over, and the rest starts as on a new slide.
 */
export function convertSlide(slide: Slide, type: SlideType): Slide {
  if (slide.type === type) return slide;
  const { id, notes, picture, video, sound, background } = slide;
  return setHeadline({ ...newSlide(type, id), notes, picture, video, sound, background }, slideTitle(slide));
}

/** An id no slide of `quiz` has yet. */
export function freshSlideId(quiz: Quiz, random: () => string = () => Math.random().toString(36).slice(2, 10)): string {
  const taken = new Set(quiz.slides.map((slide) => slide.id));
  for (;;) {
    const id = `s-${random()}`;
    if (!taken.has(id)) return id;
  }
}

/** A copy of `slide` under a new id. */
export function duplicateSlide(slide: Slide, id: string): Slide {
  return { ...slide, id };
}

export function setMode(question: QuestionSlide, mode: AnswerMode): QuestionSlide {
  if (mode === "fastestFinger") return { ...question, mode };
  // Everyone answers with the buttons, so there must be answers and a right one.
  const answers =
    question.answers.length >= MIN_EVERYONE_ANSWERS
      ? question.answers
      : [...question.answers, ...Array<string>(DEFAULT_ANSWERS - question.answers.length).fill("")];
  return { ...question, mode, answers, correct: question.correct ?? 0 };
}

export function setAnswer(question: QuestionSlide, index: number, text: string): QuestionSlide {
  return { ...question, answers: question.answers.map((answer, i) => (i === index ? text : answer)) };
}

export function addAnswer(question: QuestionSlide): QuestionSlide {
  if (question.answers.length >= MAX_ANSWERS) return question;
  return { ...question, answers: [...question.answers, ""] };
}

/**
 * Types into answer `index`. The first row after the last answer is there to
 * type a new answer into (as in the Edit question form and the table);
 * rows further down take nothing.
 */
export function setAnswerAt(question: QuestionSlide, index: number, text: string): QuestionSlide {
  if (index === question.answers.length) {
    const added = addAnswer(question);
    return added === question ? question : setAnswer(added, index, text);
  }
  return index >= 0 && index < question.answers.length ? setAnswer(question, index, text) : question;
}

/** Adds empty answers or removes the last ones until there are `count`, as far as the mode allows. */
export function withAnswerCount(question: QuestionSlide, count: number): QuestionSlide {
  let next = question;
  for (;;) {
    const step = next.answers.length < count ? addAnswer(next) : next.answers.length > count ? removeAnswer(next, next.answers.length - 1) : next;
    if (step === next) return next;
    next = step;
  }
}

/** Removes an answer; the right answer stays the same one (or falls back to the first). */
export function removeAnswer(question: QuestionSlide, index: number): QuestionSlide {
  const fewest = question.mode === "everyone" ? MIN_EVERYONE_ANSWERS : 0;
  if (question.answers.length <= fewest || index < 0 || index >= question.answers.length) return question;
  const answers = question.answers.filter((_, i) => i !== index);
  let correct = question.correct;
  if (correct === index) correct = question.mode === "everyone" ? 0 : null;
  else if (correct !== null && correct > index) correct -= 1;
  return { ...question, answers, correct };
}

/** Sets the right answer; a fastest-finger question may have none (`null`). */
export function setCorrect(question: QuestionSlide, index: number | null): QuestionSlide {
  if (index === null ? question.mode === "everyone" : index < 0 || index >= question.answers.length) return question;
  return { ...question, correct: index };
}

type Scoring = Pick<QuestionSlide, "timeLimitS" | "points" | "pointsEnd" | "penalty">;

/** Longest clock, in seconds. */
export const MAX_TIME_LIMIT_S = 3600;
/** Largest number of points either way; far inside the backend's 32-bit range. */
export const MAX_POINTS = 100_000;

/** Sets the clock and points, kept to whole numbers in range. */
export function setScoring(question: QuestionSlide, scoring: Partial<Scoring>): QuestionSlide {
  const next = { ...question, ...scoring };
  return {
    ...next,
    timeLimitS: clampInt(next.timeLimitS, 0, MAX_TIME_LIMIT_S),
    points: clampInt(next.points, -MAX_POINTS, MAX_POINTS),
    pointsEnd: next.pointsEnd === null ? null : clampInt(next.pointsEnd, -MAX_POINTS, MAX_POINTS),
    penalty: clampInt(next.penalty, 0, MAX_POINTS),
  };
}

/** A whole number in [min, max]; 0 (inside every range used here) for an empty field. */
function clampInt(value: number, min: number, max: number): number {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : 0;
}

const NEW_VIDEO: Omit<VideoClip, "file"> = { autoplay: true, muted: false, repeat: false };
const NEW_SOUND: Omit<SoundClip, "file"> = { start: "onSlide", repeat: false, volume: 100 };

/** Shows a picture; a slide shows a picture or a video, so the video goes. */
export function setPicture(slide: Slide, file: string | null): Slide {
  return { ...slide, picture: file, video: file === null ? slide.video : null };
}

/** Shows a video (keeping its options when the file is replaced); the picture goes. */
export function setVideo(slide: Slide, file: string | null): Slide {
  if (file === null) return { ...slide, video: null };
  return { ...slide, video: { ...(slide.video ?? NEW_VIDEO), file }, picture: null };
}

export function setVideoOptions(slide: Slide, options: Partial<Omit<VideoClip, "file">>): Slide {
  return slide.video ? { ...slide, video: { ...slide.video, ...options } } : slide;
}

/** Plays a sound (keeping its options when the file is replaced). */
export function setSound(slide: Slide, file: string | null): Slide {
  if (file === null) return { ...slide, sound: null };
  return { ...slide, sound: { ...(slide.sound ?? NEW_SOUND), file } };
}

export function setSoundOptions(slide: Slide, options: Partial<Omit<SoundClip, "file">>): Slide {
  if (!slide.sound) return slide;
  const volume = Math.round(Math.min(100, Math.max(0, options.volume ?? slide.sound.volume)));
  return { ...slide, sound: { ...slide.sound, ...options, volume } };
}

export function setBackground(slide: Slide, file: string | null): Slide {
  return { ...slide, background: file };
}
