// The editable properties of a slide, each a labelled control with a
// description. The properties panel and the ribbon lay out the same rows.

import type { ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { MAX_ANSWERS, MAX_POINTS, MAX_TIME_LIMIT_S, MIN_EVERYONE_ANSWERS } from "../bindings/Constants";
import type { StringKey, Translate } from "../i18n";
import { SoundIcon } from "../icons";
import type { MediaKind } from "../media";
import { SLIDE_TYPE_LABEL, type SlideContext } from "../quiz";
import type { QuestionSlide, Slide, SlideType, SoundStart } from "../types";
import { MediaPicker, NumberBox, Select, Switch } from "./controls";
import {
  convertSlide,
  removeAnswer,
  setAnswer,
  setBackground,
  setCorrect,
  setMode,
  setPicture,
  setScoring,
  setSound,
  setSoundOptions,
  setVideo,
  setVideoOptions,
  withAnswerCount,
} from "./slides";

/** What the properties need to change the selected slide. */
export interface SlideEditing {
  slide: Slide;
  /** What the slide shows from the rest of the quiz. */
  context: SlideContext;
  /** Changes the slide; `field` names what is typed into, so quick typing is one undo step. */
  update: (change: (slide: Slide) => Slide, field?: string) => void;
  /** Asks for a media file and applies it to the slide as it is by then. */
  chooseMedia: (kinds: MediaKind[], apply: (slide: Slide, file: string) => Slide) => void;
}

export interface PropertyRow {
  label: string;
  description: string;
  control: ReactNode;
  /** The control spans the panel's width, under its label. */
  wide?: boolean;
}

const SLIDE_TYPES: SlideType[] = ["question", "billboard", "explanation", "endOfRound"];

/** Every property row of the slide being edited. */
export function slideProperties(t: Translate, { slide, context, update, chooseMedia }: SlideEditing) {
  const question = (change: (q: QuestionSlide) => QuestionSlide, field?: string) => update((s) => (s.type === "question" ? change(s) : s), field);
  const row = (label: StringKey, description: StringKey, control: ReactNode, wide = false): PropertyRow => ({
    label: t(label),
    description: t(description),
    control,
    wide,
  });
  const textArea = (label: StringKey, value: string, onChange: (text: string) => void) => (
    <textarea rows={3} aria-label={t(label)} value={value} onChange={(e) => onChange(e.target.value)} />
  );
  const soundStarts: [SoundStart, string][] = [
    ["onSlide", t("soundOnSlide")],
    ["withClock", t("soundWithClock")],
    ["onDemand", t("soundOnDemand")],
  ];

  return {
    type: row(
      "propType",
      "descType",
      <Select
        label={t("propType")}
        value={slide.type}
        options={SLIDE_TYPES.map((type) => [type, t(SLIDE_TYPE_LABEL[type])])}
        onChange={(type) => update((s) => convertSlide(s, type))}
      />,
    ),
    id: row("propId", "descId", <code className="props-readonly">{slide.id}</code>),

    answerTime: (q: QuestionSlide) =>
      row(
        "propAnswerTime",
        "descAnswerTime",
        <NumberBox
          label={t("propAnswerTime")}
          value={q.timeLimitS}
          min={0}
          max={MAX_TIME_LIMIT_S}
          unit="s"
          onChange={(v) => question((s) => setScoring(s, { timeLimitS: v }), "timeLimitS")}
        />,
      ),
    everyoneAnswers: (q: QuestionSlide) =>
      row(
        "propEveryoneAnswers",
        "descEveryoneAnswers",
        <Switch
          label={t("propEveryoneAnswers")}
          checked={q.mode === "everyone"}
          onChange={(on) => question((s) => setMode(s, on ? "everyone" : "fastestFinger"))}
        />,
      ),
    answerCount: (q: QuestionSlide) => {
      const fewest = q.mode === "everyone" ? MIN_EVERYONE_ANSWERS : 0;
      const counts = Array.from({ length: MAX_ANSWERS - fewest + 1 }, (_, i) => fewest + i);
      return row(
        "propAnswerCount",
        "descAnswerCount",
        <Select
          label={t("propAnswerCount")}
          value={q.answers.length}
          options={counts.map((n) => [n, String(n)])}
          onChange={(n) => question((s) => withAnswerCount(s, n))}
        />,
      );
    },
    correctAnswer: (q: QuestionSlide) => {
      const none: [number, string][] = q.mode === "fastestFinger" ? [[-1, t("noCorrectAnswer")]] : [];
      return row(
        "propCorrectAnswer",
        "descCorrectAnswer",
        <Select
          label={t("propCorrectAnswer")}
          value={q.correct ?? -1}
          options={[...none, ...q.answers.map((_, i): [number, string] => [i, ANSWER_LETTERS[i]])]}
          disabled={q.answers.length === 0}
          onChange={(i) => question((s) => setCorrect(s, i < 0 ? null : i))}
        />,
      );
    },

    pointsStart: (q: QuestionSlide) =>
      row(
        "propPointsStart",
        "descPointsStart",
        <NumberBox
          label={t("propPointsStart")}
          value={q.points}
          min={-MAX_POINTS}
          max={MAX_POINTS}
          onChange={(v) => question((s) => setScoring(s, { points: v }), "points")}
        />,
      ),
    falling: (q: QuestionSlide) =>
      row(
        "propFalling",
        "descFalling",
        <Switch
          label={t("propFalling")}
          checked={q.pointsEnd !== null}
          onChange={(on) => question((s) => setScoring(s, { pointsEnd: on ? Math.round(s.points / 2) : null }))}
        />,
      ),
    pointsEnd: (q: QuestionSlide) =>
      row(
        "propPointsEnd",
        "descPointsEnd",
        <NumberBox
          label={t("propPointsEnd")}
          value={q.pointsEnd ?? q.points}
          min={-MAX_POINTS}
          max={MAX_POINTS}
          onChange={(v) => question((s) => (s.pointsEnd === null ? s : setScoring(s, { pointsEnd: v })), "pointsEnd")}
        />,
      ),
    penalty: (q: QuestionSlide) =>
      row(
        "propPenalty",
        "descPenalty",
        <NumberBox
          label={t("propPenalty")}
          value={q.penalty}
          min={0}
          max={MAX_POINTS}
          onChange={(v) => question((s) => setScoring(s, { penalty: v }), "penalty")}
        />,
      ),

    questionText: (q: QuestionSlide) =>
      row(
        "questionField",
        "descQuestionText",
        textArea("questionField", q.text, (text) => question((s) => ({ ...s, text }), "text")),
        true,
      ),
    title: (s: Extract<Slide, { type: "billboard" | "endOfRound" }>) =>
      row(
        "slideTitleField",
        "descTitle",
        textArea("slideTitleField", s.title, (title) => update((x) => (x.type === "billboard" || x.type === "endOfRound" ? { ...x, title } : x), "title")),
        true,
      ),
    bodyText: (s: Extract<Slide, { type: "billboard" }>) =>
      row(
        "slideTextField",
        "descBodyText",
        textArea("slideTextField", s.text, (text) => update((x) => (x.type === "billboard" ? { ...x, text } : x), "text")),
        true,
      ),
    explanationText: (s: Extract<Slide, { type: "explanation" }>) =>
      row(
        "explanationField",
        "descExplanation",
        textArea("explanationField", s.text, (text) => update((x) => (x.type === "explanation" ? { ...x, text } : x), "text")),
        true,
      ),
    /** Which question an explanation slide explains; set by where it stands. */
    explains: row(
      "explainsField",
      "descExplains",
      <span className="props-readonly">
        {context.explains ? `${t("questionNumber", { n: context.explains.position.n })}: ${context.explains.question.text}` : t("noQuestionToExplain")}
      </span>,
      true,
    ),
    notes: row(
      "notes",
      "descNotes",
      textArea("notes", slide.notes, (notes) => update((s) => ({ ...s, notes }), "notes")),
      true,
    ),

    answerText: (q: QuestionSlide, i: number) =>
      row(
        "propAnswerText",
        "descAnswerText",
        textArea("propAnswerText", q.answers[i] ?? "", (text) => question((s) => setAnswer(s, i, text), `answer${i}`)),
        true,
      ),
    isCorrect: (q: QuestionSlide, i: number) =>
      row(
        "propIsCorrect",
        "descIsCorrect",
        <Switch
          label={t("propIsCorrect")}
          checked={q.correct === i}
          disabled={q.correct === i && q.mode === "everyone"}
          onChange={(on) => question((s) => setCorrect(s, on ? i : null))}
        />,
      ),
    removeAnswer: (q: QuestionSlide, i: number) =>
      row(
        "removeAnswer",
        "descRemoveAnswer",
        <button type="button" className="props-button" disabled={removeAnswer(q, i) === q} onClick={() => question((s) => removeAnswer(s, i))}>
          {t("remove")}
        </button>,
      ),

    picture: row(
      "propPicture",
      "descPicture",
      <MediaPicker
        label={t("propPicture")}
        file={slide.picture}
        onChoose={() => chooseMedia(["image"], setPicture)}
        onRemove={() => update((s) => setPicture(s, null))}
      />,
    ),
    video: row(
      "propVideo",
      "descVideo",
      <MediaPicker
        label={t("propVideo")}
        file={slide.video?.file ?? null}
        onChoose={() => chooseMedia(["video"], setVideo)}
        onRemove={() => update((s) => setVideo(s, null))}
      />,
    ),
    /** Options of the slide's video; none without one. */
    videoOptions: slide.video
      ? [
          row(
            "videoAutoplay",
            "descVideoAutoplay",
            <Switch label={t("videoAutoplay")} checked={slide.video.autoplay} onChange={(on) => update((s) => setVideoOptions(s, { autoplay: on }))} />,
          ),
          row(
            "videoMuted",
            "descVideoMuted",
            <Switch label={t("videoMuted")} checked={slide.video.muted} onChange={(on) => update((s) => setVideoOptions(s, { muted: on }))} />,
          ),
          row(
            "repeat",
            "descVideoRepeat",
            <Switch label={t("repeat")} checked={slide.video.repeat} onChange={(on) => update((s) => setVideoOptions(s, { repeat: on }))} />,
          ),
        ]
      : [],
    sound: row(
      "propSound",
      "descSound",
      <MediaPicker
        label={t("propSound")}
        file={slide.sound?.file ?? null}
        onChoose={() => chooseMedia(["audio"], setSound)}
        onRemove={() => update((s) => setSound(s, null))}
      />,
    ),
    /** Options of the slide's sound; none without one. */
    soundOptions: slide.sound
      ? [
          row(
            "soundStart",
            "descSoundStart",
            <Select label={t("soundStart")} value={slide.sound.start} options={soundStarts} onChange={(start) => update((s) => setSoundOptions(s, { start }))} />,
          ),
          row(
            "repeat",
            "descSoundRepeat",
            <Switch label={t("repeat")} checked={slide.sound.repeat} onChange={(on) => update((s) => setSoundOptions(s, { repeat: on }))} />,
          ),
          row(
            "volume",
            "descVolume",
            <span className="volume">
              <SoundIcon />
              <input
                type="range"
                min={0}
                max={100}
                aria-label={t("volume")}
                value={slide.sound.volume}
                onChange={(e) => {
                  const volume = Number(e.target.value);
                  update((s) => setSoundOptions(s, { volume }), "volume");
                }}
              />
              <span className="digits">{slide.sound.volume} %</span>
            </span>,
          ),
        ]
      : [],
    background: row(
      "propBackground",
      "descBackground",
      <MediaPicker
        label={t("propBackground")}
        file={slide.background}
        onChoose={() => chooseMedia(["image", "video"], setBackground)}
        onRemove={() => update((s) => setBackground(s, null))}
      />,
    ),
  };
}

export type SlideProperties = ReturnType<typeof slideProperties>;
