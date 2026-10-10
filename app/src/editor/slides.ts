// Changes to one slide. Each keeps the slide playable: the editor offers no
// way to make a question the show would refuse (see `Quiz::validate` in
// crates/quiz-core/src/model.rs).

import { WHOLE_FILE } from "../audio/clip-player";
import {
  DEFAULT_POINTS,
  DEFAULT_TIME_LIMIT_S,
  MAX_ANSWERS,
  MAX_EVERYONE_ANSWERS,
  MAX_GAIN_DB,
  MAX_INTRO_PAUSE_MS,
  MAX_POINTS,
  MAX_REPLAY_DELAY_MS,
  MAX_TIME_LIMIT_S,
  MAX_VIDEO_SPEED,
  MIN_EVERYONE_ANSWERS,
  MIN_GAIN_DB,
  MIN_VIDEO_SPEED,
} from "../bindings/Constants";
import { hasTitle, showsRanking, slideTitle } from "../quiz";
import type { AnswerMode, Background, Quiz, QuestionSlide, Replay, Slide, SlideType, SoundClip, Span, VideoClip } from "../types";

/** Answers a new everyone-answers question starts with: one per buzzer button. */
const DEFAULT_ANSWERS = MAX_EVERYONE_ANSWERS;

/**
 * Fewest and most answers a question can have in `mode`
 * (`Question::problems` in crates/quiz-core/src/model.rs): everyone answers
 * with a button, so 2 to 4; a fastest finger question is answered aloud, so
 * none to all six letters.
 */
export function answerLimits(mode: AnswerMode): { fewest: number; most: number } {
  return mode === "everyone" ? { fewest: MIN_EVERYONE_ANSWERS, most: MAX_EVERYONE_ANSWERS } : { fewest: 0, most: MAX_ANSWERS };
}

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
        timeLimitS: DEFAULT_TIME_LIMIT_S,
        points: DEFAULT_POINTS,
        pointsEnd: null,
        penalty: 0,
      };
    case "explanation":
      return { ...base, type, text: "" };
    case "endOfRound":
    case "final":
      return { ...base, type, title: "" };
  }
}

/** Sets the text the slide is listed by (see `slideTitle`). */
export function setHeadline(slide: Slide, text: string): Slide {
  return hasTitle(slide) ? { ...slide, title: text } : { ...slide, text };
}

/** Sets the text under a billboard's title; other slides have none. */
export function setBodyText(slide: Slide, text: string): Slide {
  return slide.type === "billboard" ? { ...slide, text } : slide;
}

export function setNotes(slide: Slide, notes: string): Slide {
  return { ...slide, notes };
}

/**
 * The slide as another type. Its id, notes and media stay, the question,
 * explanation or title carries over, and the rest starts as on a new slide.
 * Only a question reveals an answer, so only a question keeps the replay
 * parts of its sound and video; only a question has the clock a sound can
 * play with (elsewhere it plays when the slide opens) and the answer time a
 * video effect follows. A ranking shows no picture or video, so they go.
 */
export function convertSlide(slide: Slide, type: SlideType): Slide {
  if (slide.type === type) return slide;
  const { id, notes, background } = slide;
  const question = type === "question";
  const sound = slide.sound && !question ? { ...slide.sound, start: slide.sound.start === "withClock" ? "onSlide" : slide.sound.start, replay: null } : slide.sound;
  const video = slide.video && !question ? { ...slide.video, replay: null, effect: null } : slide.video;
  const media = showsRanking(type) ? { picture: null, video: null } : { picture: slide.picture, video };
  return setHeadline({ ...newSlide(type, id), notes, ...media, sound, background }, slideTitle(slide));
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
  // Everyone answers with the buttons: an answer per button at most, and a right one.
  const { fewest, most } = answerLimits(mode);
  const count = question.answers.length;
  const answers =
    count < fewest ? [...question.answers, ...Array<string>(DEFAULT_ANSWERS - count).fill("")] : question.answers.slice(0, most);
  const correct = question.correct !== null && question.correct < answers.length ? question.correct : 0;
  return { ...question, mode, answers, correct };
}

export function setAnswer(question: QuestionSlide, index: number, text: string): QuestionSlide {
  return { ...question, answers: question.answers.map((answer, i) => (i === index ? text : answer)) };
}

export function addAnswer(question: QuestionSlide): QuestionSlide {
  if (question.answers.length >= answerLimits(question.mode).most) return question;
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
  if (question.answers.length <= answerLimits(question.mode).fewest || index < 0 || index >= question.answers.length) return question;
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

/** A whole number in [min, max]; an empty field counts as 0. */
function clampInt(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? Math.round(value) : 0));
}

/** The options a quiz file gets when it names only the file (crates/quiz-core/src/media.rs). */
const NEW_VIDEO: Omit<VideoClip, "file"> = {
  autoplay: true,
  introPauseMs: 1_000,
  muted: false,
  repeat: false,
  volume: 100,
  speed: 100,
  part: WHOLE_FILE,
  effect: null,
  replay: null,
};
const NEW_SOUND: Omit<SoundClip, "file"> = { start: "onSlide", repeat: false, volume: 100, part: WHOLE_FILE, gainDb: 0, replay: null };
const NEW_BACKGROUND: Omit<Background, "file"> = { repeat: true, muted: true, sync: false };

/** Shows a picture; a slide shows a picture or a video, so the video goes. */
export function setPicture(slide: Slide, file: string | null): Slide {
  return { ...slide, picture: file, video: file === null ? slide.video : null };
}

/** Shows a video (keeping its options when the file is replaced); the picture goes. */
export function setVideo(slide: Slide, file: string | null): Slide {
  if (file === null) return { ...slide, video: null };
  return { ...slide, video: { ...(slide.video ?? NEW_VIDEO), file }, picture: null };
}

/**
 * Changes the video's options, kept playable (see `playable`): speed and
 * intro pause within their limits, an effect on questions only.
 */
export function setVideoOptions(slide: Slide, options: Partial<Omit<VideoClip, "file">>): Slide {
  const video = slide.video && playable(slide, { ...slide.video, ...options });
  if (!video) return slide;
  const speed = clampInt(video.speed, MIN_VIDEO_SPEED, MAX_VIDEO_SPEED);
  const introPauseMs = clampInt(video.introPauseMs, 0, MAX_INTRO_PAUSE_MS);
  return { ...slide, video: { ...video, speed, introPauseMs, effect: slide.type === "question" ? video.effect : null } };
}

/** Plays a sound (keeping its options when the file is replaced). */
export function setSound(slide: Slide, file: string | null): Slide {
  if (file === null) return { ...slide, sound: null };
  return { ...slide, sound: { ...(slide.sound ?? NEW_SOUND), file } };
}

/**
 * Changes the sound's options, kept playable (see `playable`): loudness
 * gain within its limits to 0.1 dB.
 */
export function setSoundOptions(slide: Slide, options: Partial<Omit<SoundClip, "file">>): Slide {
  const sound = slide.sound && playable(slide, { ...slide.sound, ...options });
  if (!sound) return slide;
  const gainDb = Math.round(Math.min(MAX_GAIN_DB, Math.max(MIN_GAIN_DB, Number.isFinite(sound.gainDb) ? sound.gainDb : 0)) * 10) / 10;
  return { ...slide, sound: { ...sound, gainDb } };
}

/**
 * A sound's or video's options as the show takes them (`playback_problems`
 * in crates/quiz-core/src/model.rs): volume a whole percentage, parts in
 * whole milliseconds, a replay part on questions only that starts within
 * its limit. Null when a part would hold nothing.
 */
function playable<T extends { volume: number; part: Span; replay: Replay | null }>(slide: Slide, clip: T): T | null {
  const part = wholeMs(clip.part);
  const replay = slide.type === "question" && clip.replay ? { part: wholeMs(clip.replay.part), delayMs: clampInt(clip.replay.delayMs, 0, MAX_REPLAY_DELAY_MS) } : null;
  if (isEmptySpan(part) || (replay && isEmptySpan(replay.part))) return null;
  return { ...clip, volume: clampInt(clip.volume, 0, 100), part, replay };
}

/** A replay part as it is first ticked: the part that plays, at once after the reveal. */
export function newReplay(part: Span): Replay {
  return { part, delayMs: 0 };
}

/** `span` in whole, non-negative milliseconds. */
function wholeMs(span: Span): Span {
  return { fromMs: Math.max(0, Math.round(span.fromMs)), toMs: span.toMs === null ? null : Math.max(0, Math.round(span.toMs)) };
}

/** Holds nothing: ends where or before it starts (`Span::is_empty` in Rust). */
export function isEmptySpan(span: Span): boolean {
  return span.toMs !== null && span.toMs <= span.fromMs;
}

/** Shows a picture or video behind the slide (keeping its options when the file is replaced). */
export function setBackground(slide: Slide, file: string | null): Slide {
  return { ...slide, background: file === null ? null : { ...(slide.background ?? NEW_BACKGROUND), file } };
}

export function setBackgroundOptions(slide: Slide, options: Partial<Omit<Background, "file">>): Slide {
  return slide.background ? { ...slide, background: { ...slide.background, ...options } } : slide;
}
