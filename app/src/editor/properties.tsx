// The editable properties of a slide, each a labelled control with a
// description. The properties panel and the ribbon lay out the same rows.

import type { ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import {
  MAX_GAIN_DB,
  MAX_INTRO_PAUSE_MS,
  MAX_POINTS,
  MAX_REPLAY_DELAY_MS,
  MAX_TIME_LIMIT_S,
  MAX_VIDEO_SPEED,
  MIN_GAIN_DB,
  MIN_VIDEO_SPEED,
} from "../bindings/Constants";
import { sameSpan, WHOLE_FILE } from "../audio/clip-player";
import { trackTime } from "../format";
import type { StringKey, Translate } from "../i18n";
import { SoundIcon } from "../icons";
import { isVideo, type MediaKind } from "../media";
import { SLIDE_TYPE_LABEL, type SlideContext } from "../quiz";
import type { QuestionSlide, Replay, Slide, SlideType, SoundClip, SoundStart, Span, TitledSlide, VideoClip, VideoEffect } from "../types";
import { MediaPicker, NumberBox, Select, Switch } from "./controls";
import {
  answerLimits,
  convertSlide,
  newReplay,
  removeAnswer,
  setAnswer,
  setBackground,
  setBackgroundOptions,
  setBodyText,
  setCorrect,
  setHeadline,
  setMode,
  setNotes,
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
  /** Opens the trim window for the slide's sound. */
  trimSound: () => void;
  /** Opens the trim window for the slide's video. */
  trimVideo: () => void;
  /** Measures the slide's sound and sets its loudness gain to the remembered target. */
  normalizeSound: () => void;
}

export interface PropertyRow {
  label: string;
  description: string;
  control: ReactNode;
  /** The control spans the panel's width, under its label. */
  wide?: boolean;
}

const SLIDE_TYPES: SlideType[] = ["question", "billboard", "explanation", "endOfRound", "final"];

/** Every property row of the slide being edited. */
export function slideProperties(t: Translate, { slide, context, update, chooseMedia, trimSound, trimVideo, normalizeSound }: SlideEditing) {
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
  // Only a question has a clock (a file may still say so of another slide).
  const soundStarts: [SoundStart, string][] = [
    ["onSlide", t("soundOnSlide")],
    ...(slide.type === "question" || slide.sound?.start === "withClock" ? [["withClock", t("soundWithClock")] as [SoundStart, string]] : []),
    ["onDemand", t("soundOnDemand")],
  ];
  const spanText = (span: Span) =>
    sameSpan(span, WHOLE_FILE) ? t("wholeTrack") : `${trackTime(span.fromMs)} – ${span.toMs === null ? t("trackEnd") : trackTime(span.toMs)}`;
  const sound = slide.sound;
  const video = slide.video;
  const backgroundVideo = slide.background && isVideo(slide.background.file) ? slide.background : null;
  const effects: [VideoEffect | "none", string][] = [
    ["none", t("effectNone")],
    ["blur", t("effectBlur")],
    ["mosaic", t("effectMosaic")],
  ];

  /** Changes what a sound and a video have in common, on the slide as it is by then. */
  const playback = (key: "sound" | "video", change: (clip: SoundClip | VideoClip) => Partial<Pick<SoundClip, "volume" | "replay">>, field?: string) =>
    update((s) => {
      const clip = s[key];
      if (!clip) return s;
      return key === "sound" ? setSoundOptions(s, change(clip)) : setVideoOptions(s, change(clip));
    }, field && `${key}-${field}`);
  const volume = (key: "sound" | "video", clip: SoundClip | VideoClip, description: StringKey) =>
    row(
      "volume",
      description,
      <span className="volume">
        <SoundIcon />
        <input
          type="range"
          min={0}
          max={100}
          aria-label={t("volume")}
          value={clip.volume}
          onChange={(e) => {
            const value = Number(e.target.value);
            playback(key, () => ({ volume: value }), "volume");
          }}
        />
        <span className="tabular">{clip.volume} %</span>
      </span>,
    );
  const part = (clip: SoundClip | VideoClip, description: StringKey, trim: () => void) =>
    row(
      "propSoundPart",
      description,
      <span className="props-inline">
        <span className="tabular">{spanText(clip.part)}</span>
        <button type="button" className="props-button" onClick={trim}>
          {t("trimAction")}
        </button>
      </span>,
    );
  /** Replaying a part after the reveal; questions only. */
  const replay = (key: "sound" | "video", clip: { replay: Replay | null }, description: StringKey): PropertyRow[] =>
    slide.type !== "question"
      ? []
      : [
          row(
            "propReplay",
            description,
            <Switch label={t("propReplay")} checked={clip.replay !== null} onChange={(on) => playback(key, (c) => ({ replay: on ? newReplay(c.part) : null }))} />,
          ),
          ...(clip.replay
            ? [
                row("trimReplayPart", description, <span className="tabular props-readonly">{spanText(clip.replay.part)}</span>),
                row(
                  "propReplayDelay",
                  "descReplayDelay",
                  <NumberBox
                    label={t("propReplayDelay")}
                    value={clip.replay.delayMs / 1000}
                    min={0}
                    max={MAX_REPLAY_DELAY_MS / 1000}
                    step={0.1}
                    unit="s"
                    onChange={(seconds) => playback(key, (c) => ({ replay: c.replay && { ...c.replay, delayMs: seconds * 1000 } }), "replayDelay")}
                  />,
                ),
              ]
            : []),
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
      const { fewest, most } = answerLimits(q.mode);
      // A file from an older version may have more; it is offered until changed.
      const counts = Array.from({ length: Math.max(most, q.answers.length) - fewest + 1 }, (_, i) => fewest + i);
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
        textArea("questionField", q.text, (text) => update((s) => setHeadline(s, text), "text")),
        true,
      ),
    title: (s: TitledSlide) =>
      row(
        "slideTitleField",
        "descTitle",
        textArea("slideTitleField", s.title, (title) => update((x) => setHeadline(x, title), "title")),
        true,
      ),
    bodyText: (s: Extract<Slide, { type: "billboard" }>) =>
      row(
        "slideTextField",
        "descBodyText",
        textArea("slideTextField", s.text, (text) => update((x) => setBodyText(x, text), "text")),
        true,
      ),
    explanationText: (s: Extract<Slide, { type: "explanation" }>) =>
      row(
        "explanationField",
        "descExplanation",
        textArea("explanationField", s.text, (text) => update((x) => setHeadline(x, text), "text")),
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
      textArea("notes", slide.notes, (notes) => update((s) => setNotes(s, notes), "notes")),
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
    videoOptions: video
      ? [
          row("videoAutoplay", "descVideoAutoplay", <Switch label={t("videoAutoplay")} checked={video.autoplay} onChange={(on) => update((s) => setVideoOptions(s, { autoplay: on }))} />),
          ...(video.autoplay
            ? [
                row(
                  "propIntroPause",
                  "descIntroPause",
                  <NumberBox
                    label={t("propIntroPause")}
                    value={video.introPauseMs / 1000}
                    min={0}
                    max={MAX_INTRO_PAUSE_MS / 1000}
                    step={0.1}
                    unit="s"
                    onChange={(seconds) => update((s) => setVideoOptions(s, { introPauseMs: seconds * 1000 }), "introPause")}
                  />,
                ),
              ]
            : []),
          row("videoMuted", "descVideoMuted", <Switch label={t("videoMuted")} checked={video.muted} onChange={(on) => update((s) => setVideoOptions(s, { muted: on }))} />),
          row("repeat", "descVideoRepeat", <Switch label={t("repeat")} checked={video.repeat} onChange={(on) => update((s) => setVideoOptions(s, { repeat: on }))} />),
          volume("video", video, "descVideoVolume"),
          row(
            "propSpeed",
            "descSpeed",
            <NumberBox
              label={t("propSpeed")}
              value={video.speed}
              min={MIN_VIDEO_SPEED}
              max={MAX_VIDEO_SPEED}
              step={5}
              unit="%"
              onChange={(speed) => update((s) => setVideoOptions(s, { speed }), "speed")}
            />,
          ),
        ]
      : [],
    /** Which part of the video plays; null without one. */
    videoPart: video && part(video, "descVideoPart", trimVideo),
    /** The part a question's video replays after the reveal. */
    videoReplay: video ? replay("video", video, "descVideoReplay") : [],
    /** The effect over a question's video; null on other slides or without a video. */
    videoEffect:
      video && slide.type === "question"
        ? row(
            "propEffect",
            "descEffect",
            <Select
              label={t("propEffect")}
              value={video.effect ?? "none"}
              options={effects}
              onChange={(effect) => update((s) => setVideoOptions(s, { effect: effect === "none" ? null : effect }))}
            />,
          )
        : null,
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
          volume("sound", slide.sound, "descVolume"),
        ]
      : [],
    /** Which part of the sound plays; null without one. */
    soundPart: sound && part(sound, "descSoundPart", trimSound),
    /** The part a question's sound replays after the reveal. */
    replay: sound ? replay("sound", sound, "descReplay") : [],
    /** The sound's loudness gain; null without a sound. */
    gain:
      sound &&
      row(
        "propGain",
        "descGain",
        <NumberBox
          label={t("propGain")}
          value={sound.gainDb}
          min={MIN_GAIN_DB}
          max={MAX_GAIN_DB}
          step={0.1}
          unit="dB"
          onChange={(gainDb) => update((s) => setSoundOptions(s, { gainDb }), "gainDb")}
        />,
      ),
    /** Normalizing the sound; null without one. The ribbon has its own button. */
    normalize:
      sound &&
      row(
        "normalizeAction",
        "descNormalize",
        <button type="button" className="props-button" onClick={normalizeSound}>
          {t("normalizeAction")}
        </button>,
      ),
    background: row(
      "propBackground",
      "descBackground",
      <MediaPicker
        label={t("propBackground")}
        file={slide.background?.file ?? null}
        onChoose={() => chooseMedia(["image", "video"], setBackground)}
        onRemove={() => update((s) => setBackground(s, null))}
      />,
    ),
    /** Options of a background video; none for a picture or without a background. */
    backgroundOptions: backgroundVideo
      ? [
          row(
            "repeat",
            "descBackgroundRepeat",
            <Switch label={t("repeat")} checked={backgroundVideo.repeat} onChange={(on) => update((s) => setBackgroundOptions(s, { repeat: on }))} />,
          ),
          row(
            "videoMuted",
            "descBackgroundMuted",
            <Switch label={t("videoMuted")} checked={backgroundVideo.muted} onChange={(on) => update((s) => setBackgroundOptions(s, { muted: on }))} />,
          ),
          row(
            "propBackgroundSync",
            "descBackgroundSync",
            <Switch label={t("propBackgroundSync")} checked={backgroundVideo.sync} onChange={(on) => update((s) => setBackgroundOptions(s, { sync: on }))} />,
          ),
        ]
      : [],
  };
}

export type SlideProperties = ReturnType<typeof slideProperties>;
