// The editable properties of a slide, each a labelled control with a
// description. The properties panel and the ribbon lay out the same rows.

import {
  DEFAULT_POINTS,
  DEFAULT_TIME_LIMIT_S,
  FULL_SHARE,
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
import { isVideo, mediaKindOf, type MediaKind } from "../media";
import { isCorrect, shareOf, SLIDE_TYPE_LABEL, type SlideContext } from "../quiz";
import type { QuestionSlide, Replay, Slide, SlideType, SoundClip, SoundStart, Span, TitledSlide, VideoClip, VideoEffect } from "../types";
import { AnswerToggles, MediaPicker } from "./controls";
import type { PropertyRow } from "./property-rows";
import { rowBuilders } from "./row-builders";
import {
  answerLimits,
  convertSlide,
  NEW_BACKGROUND,
  NEW_SOUND,
  NEW_VIDEO,
  newReplay,
  removeAnswer,
  setAnswer,
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
  setSound,
  setSoundOptions,
  setVideo,
  setVideoOptions,
  toggleCorrect,
  withAnswerCount,
} from "./slides";

export type { PropertyRow } from "./property-rows";

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

const SLIDE_TYPES: SlideType[] = ["question", "billboard", "explanation", "endOfRound", "final"];

/** Every property row of the slide being edited. */
export function slideProperties(t: Translate, { slide, context, update, chooseMedia, trimSound, trimVideo, normalizeSound }: SlideEditing) {
  const question = (change: (q: QuestionSlide) => QuestionSlide, field?: string) => update((s) => (s.type === "question" ? change(s) : s), field);
  const { row, numberRow, flagRow, choiceRow } = rowBuilders(t);
  /** A media file of the slide: choose, remove, and Copy and Paste between slides. */
  const mediaRow = (id: string, label: StringKey, description: StringKey, file: string | null, kinds: MediaKind[], apply: (s: Slide, file: string | null) => Slide) =>
    row(
      id,
      label,
      description,
      <MediaPicker label={t(label)} file={file} onChoose={() => chooseMedia(kinds, apply)} onRemove={() => update((s) => apply(s, null))} />,
      {
        value: {
          kind: "media",
          value: file,
          initial: null,
          accepts: (pasted) => {
            const kind = mediaKindOf(pasted);
            return kind !== null && kinds.includes(kind);
          },
          set: (pasted) => update((s) => apply(s, pasted)),
        },
      },
    );
  const textArea = (label: StringKey, value: string, onChange: (text: string) => void) => (
    <textarea rows={3} aria-label={t(label)} value={value} onChange={(e) => onChange(e.target.value)} />
  );
  // Only a question has a clock.
  const soundStarts: [SoundStart, string][] = [
    ["onSlide", t("soundOnSlide")],
    ...(slide.type === "question" ? [["withClock", t("soundWithClock")] as [SoundStart, string]] : []),
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
    numberRow(`${key}.volume`, "volume", description, {
      value: clip.volume,
      initial: 100,
      min: 0,
      max: 100,
      unit: "%",
      slider: true,
      set: (value) => playback(key, () => ({ volume: value }), "volume"),
    });
  const part = (key: "sound" | "video", clip: SoundClip | VideoClip, description: StringKey, trim: () => void) =>
    row(
      `${key}.part`,
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
          flagRow(`${key}.replay`, "propReplay", description, {
            value: clip.replay !== null,
            initial: false,
            set: (on) => playback(key, (c) => ({ replay: on ? newReplay(c.part) : null })),
          }),
          ...(clip.replay
            ? [
                row(`${key}.replayPart`, "trimReplayPart", description, <span className="tabular props-readonly">{spanText(clip.replay.part)}</span>),
                numberRow(
                  `${key}.replayDelay`,
                  "propReplayDelay",
                  "descReplayDelay",
                  {
                    value: clip.replay.delayMs / 1000,
                    initial: 0,
                    min: 0,
                    max: MAX_REPLAY_DELAY_MS / 1000,
                    step: 0.1,
                    unit: "s",
                    set: (seconds) => playback(key, (c) => ({ replay: c.replay && { ...c.replay, delayMs: seconds * 1000 } }), "replayDelay"),
                  },
                  { rare: true },
                ),
              ]
            : []),
        ];

  return {
    type: choiceRow("type", "propType", "descType", {
      value: slide.type,
      options: SLIDE_TYPES.map((type) => [type, t(SLIDE_TYPE_LABEL[type])]),
      set: (type) => update((s) => convertSlide(s, type)),
    }),
    id: row("id", "propId", "descId", <code className="props-readonly">{slide.id}</code>),

    answerTime: (q: QuestionSlide) =>
      numberRow("answerTime", "propAnswerTime", "descAnswerTime", {
        value: q.timeLimitS,
        initial: DEFAULT_TIME_LIMIT_S,
        min: 0,
        max: MAX_TIME_LIMIT_S,
        unit: "s",
        set: (v) => question((s) => setScoring(s, { timeLimitS: v }), "timeLimitS"),
      }),
    everyoneAnswers: (q: QuestionSlide) =>
      flagRow("everyoneAnswers", "propEveryoneAnswers", "descEveryoneAnswers", {
        value: q.mode === "everyone",
        set: (on) => question((s) => setMode(s, on ? "everyone" : "fastestFinger")),
      }),
    answerCount: (q: QuestionSlide) => {
      const { fewest, most } = answerLimits(q.mode);
      // A file from an older version may have more; it is offered until changed.
      const counts = Array.from({ length: Math.max(most, q.answers.length) - fewest + 1 }, (_, i) => fewest + i);
      return choiceRow("answerCount", "propAnswerCount", "descAnswerCount", {
        value: q.answers.length,
        options: counts.map((n) => [n, String(n)]),
        set: (n) => question((s) => withAnswerCount(s, n)),
      });
    },
    correctAnswer: (q: QuestionSlide) => {
      const answers = q.correct.map((c) => c.answer);
      const set = (right: number[]) => question((s) => setCorrectAnswers(s, right));
      const control = (
        <AnswerToggles
          label={t("propCorrectAnswer")}
          count={q.answers.length}
          value={answers}
          keepOne={q.mode === "everyone"}
          none={t("noCorrectAnswer")}
          onChange={set}
        />
      );
      return row("correctAnswer", "propCorrectAnswer", "descCorrectAnswer", control, {
        value: { kind: "answers", value: answers, initial: q.answers.length > 0 ? [0] : [], set },
      });
    },

    pointsStart: (q: QuestionSlide) =>
      numberRow("pointsStart", "propPointsStart", "descPointsStart", {
        value: q.points,
        initial: DEFAULT_POINTS,
        min: -MAX_POINTS,
        max: MAX_POINTS,
        set: (v) => question((s) => setScoring(s, { points: v }), "points"),
      }),
    falling: (q: QuestionSlide) =>
      flagRow("falling", "propFalling", "descFalling", {
        value: q.pointsEnd !== null,
        initial: false,
        set: (on) => question((s) => setScoring(s, { pointsEnd: on ? Math.round(s.points / 2) : null })),
      }),
    pointsEnd: (q: QuestionSlide) =>
      numberRow("pointsEnd", "propPointsEnd", "descPointsEnd", {
        value: q.pointsEnd ?? q.points,
        min: -MAX_POINTS,
        max: MAX_POINTS,
        set: (v) => question((s) => (s.pointsEnd === null ? s : setScoring(s, { pointsEnd: v })), "pointsEnd"),
      }),
    penalty: (q: QuestionSlide) =>
      numberRow("penalty", "propPenalty", "descPenalty", {
        value: q.penalty,
        initial: 0,
        min: 0,
        max: MAX_POINTS,
        set: (v) => question((s) => setScoring(s, { penalty: v }), "penalty"),
      }),

    questionText: (q: QuestionSlide) =>
      row(
        "questionText",
        "questionField",
        "descQuestionText",
        textArea("questionField", q.text, (text) => update((s) => setHeadline(s, text), "text")),
        { wide: true },
      ),
    title: (s: TitledSlide) =>
      row(
        "title",
        "slideTitleField",
        "descTitle",
        textArea("slideTitleField", s.title, (title) => update((x) => setHeadline(x, title), "title")),
        { wide: true },
      ),
    bodyText: (s: Extract<Slide, { type: "billboard" }>) =>
      row(
        "bodyText",
        "slideTextField",
        "descBodyText",
        textArea("slideTextField", s.text, (text) => update((x) => setBodyText(x, text), "text")),
        { wide: true },
      ),
    explanationText: (s: Extract<Slide, { type: "explanation" }>) =>
      row(
        "explanationText",
        "explanationField",
        "descExplanation",
        textArea("explanationField", s.text, (text) => update((x) => setHeadline(x, text), "text")),
        { wide: true },
      ),
    /** Which question an explanation slide explains; set by where it stands. */
    explains: row(
      "explains",
      "explainsField",
      "descExplains",
      <span className="props-readonly">
        {context.explains ? `${t("questionNumber", { n: context.explains.position.n })}: ${context.explains.question.text}` : t("noQuestionToExplain")}
      </span>,
      { wide: true },
    ),
    notes: row(
      "notes",
      "notes",
      "descNotes",
      textArea("notes", slide.notes, (notes) => update((s) => setNotes(s, notes), "notes")),
      { wide: true },
    ),

    answerText: (q: QuestionSlide, i: number) =>
      row(
        `answer${i}.text`,
        "propAnswerText",
        "descAnswerText",
        textArea("propAnswerText", q.answers[i] ?? "", (text) => question((s) => setAnswer(s, i, text), `answer${i}`)),
        { wide: true },
      ),
    isCorrect: (q: QuestionSlide, i: number) =>
      flagRow(`answer${i}.correct`, "propIsCorrect", "descIsCorrect", {
        value: isCorrect(q, i),
        disabled: toggleCorrect(q, i) === q,
        set: (on) => question((s) => setCorrect(s, i, on)),
      }),
    /** The share of the points a right answer gives. */
    share: (q: QuestionSlide, i: number) =>
      numberRow(`answer${i}.share`, "propShare", "descShare", {
        value: shareOf(q, i) ?? FULL_SHARE,
        initial: FULL_SHARE,
        min: 1,
        max: FULL_SHARE,
        unit: "%",
        slider: true,
        set: (v) => question((s) => setShare(s, i, v), `answer${i}.share`),
      }),
    removeAnswer: (q: QuestionSlide, i: number) =>
      row(
        `answer${i}.remove`,
        "removeAnswer",
        "descRemoveAnswer",
        <button type="button" className="props-button" disabled={removeAnswer(q, i) === q} onClick={() => question((s) => removeAnswer(s, i))}>
          {t("remove")}
        </button>,
      ),

    picture: mediaRow("picture", "propPicture", "descPicture", slide.picture, ["image"], setPicture),
    video: mediaRow("video", "propVideo", "descVideo", slide.video?.file ?? null, ["video"], setVideo),
    /** Options of the slide's video; none without one. */
    videoOptions: video
      ? [
          flagRow("video.autoplay", "videoAutoplay", "descVideoAutoplay", {
            value: video.autoplay,
            initial: NEW_VIDEO.autoplay,
            set: (on) => update((s) => setVideoOptions(s, { autoplay: on })),
          }),
          ...(video.autoplay
            ? [
                numberRow(
                  "video.introPause",
                  "propIntroPause",
                  "descIntroPause",
                  {
                    value: video.introPauseMs / 1000,
                    initial: NEW_VIDEO.introPauseMs / 1000,
                    min: 0,
                    max: MAX_INTRO_PAUSE_MS / 1000,
                    step: 0.1,
                    unit: "s",
                    set: (seconds) => update((s) => setVideoOptions(s, { introPauseMs: seconds * 1000 }), "introPause"),
                  },
                  { rare: true },
                ),
              ]
            : []),
          flagRow("video.muted", "videoMuted", "descVideoMuted", {
            value: video.muted,
            initial: NEW_VIDEO.muted,
            set: (on) => update((s) => setVideoOptions(s, { muted: on })),
          }),
          flagRow("video.repeat", "repeat", "descVideoRepeat", {
            value: video.repeat,
            initial: NEW_VIDEO.repeat,
            set: (on) => update((s) => setVideoOptions(s, { repeat: on })),
          }),
          volume("video", video, "descVideoVolume"),
          numberRow(
            "video.speed",
            "propSpeed",
            "descSpeed",
            {
              value: video.speed,
              initial: NEW_VIDEO.speed,
              min: MIN_VIDEO_SPEED,
              max: MAX_VIDEO_SPEED,
              step: 5,
              unit: "%",
              slider: true,
              set: (speed) => update((s) => setVideoOptions(s, { speed }), "speed"),
            },
            { rare: true },
          ),
        ]
      : [],
    /** Which part of the video plays; null without one. */
    videoPart: video && part("video", video, "descVideoPart", trimVideo),
    /** The part a question's video replays after the reveal. */
    videoReplay: video ? replay("video", video, "descVideoReplay") : [],
    /** The effect over a question's video; null on other slides or without a video. */
    videoEffect:
      video && slide.type === "question"
        ? choiceRow("video.effect", "propEffect", "descEffect", {
            value: video.effect ?? "none",
            initial: "none",
            options: effects,
            set: (effect) => update((s) => setVideoOptions(s, { effect: effect === "none" ? null : effect })),
          })
        : null,
    sound: mediaRow("sound", "propSound", "descSound", slide.sound?.file ?? null, ["audio"], setSound),
    /** Options of the slide's sound; none without one. */
    soundOptions: sound
      ? [
          choiceRow("sound.start", "soundStart", "descSoundStart", {
            value: sound.start,
            initial: NEW_SOUND.start,
            options: soundStarts,
            set: (start) => update((s) => setSoundOptions(s, { start })),
          }),
          flagRow("sound.repeat", "repeat", "descSoundRepeat", {
            value: sound.repeat,
            initial: NEW_SOUND.repeat,
            set: (on) => update((s) => setSoundOptions(s, { repeat: on })),
          }),
          volume("sound", sound, "descVolume"),
        ]
      : [],
    /** Which part of the sound plays; null without one. */
    soundPart: sound && part("sound", sound, "descSoundPart", trimSound),
    /** The part a question's sound replays after the reveal. */
    replay: sound ? replay("sound", sound, "descReplay") : [],
    /** The sound's loudness gain; null without a sound. */
    gain:
      sound &&
      numberRow(
        "sound.gain",
        "propGain",
        "descGain",
        {
          value: sound.gainDb,
          initial: NEW_SOUND.gainDb,
          min: MIN_GAIN_DB,
          max: MAX_GAIN_DB,
          step: 0.1,
          unit: "dB",
          slider: true,
          set: (gainDb) => update((s) => setSoundOptions(s, { gainDb }), "gainDb"),
        },
        { rare: true },
      ),
    /** Normalizing the sound; null without one. The ribbon has its own button. */
    normalize:
      sound &&
      row(
        "sound.normalize",
        "normalizeAction",
        "descNormalize",
        <button type="button" className="props-button" onClick={normalizeSound}>
          {t("normalizeAction")}
        </button>,
      ),
    background: mediaRow("background", "propBackground", "descBackground", slide.background?.file ?? null, ["image", "video"], setBackground),
    /** Options of a background video; none for a picture or without a background. */
    backgroundOptions: backgroundVideo
      ? [
          flagRow("background.repeat", "repeat", "descBackgroundRepeat", {
            value: backgroundVideo.repeat,
            initial: NEW_BACKGROUND.repeat,
            set: (on) => update((s) => setBackgroundOptions(s, { repeat: on })),
          }),
          flagRow("background.muted", "videoMuted", "descBackgroundMuted", {
            value: backgroundVideo.muted,
            initial: NEW_BACKGROUND.muted,
            set: (on) => update((s) => setBackgroundOptions(s, { muted: on })),
          }),
          flagRow(
            "background.sync",
            "propBackgroundSync",
            "descBackgroundSync",
            {
              value: backgroundVideo.sync,
              initial: NEW_BACKGROUND.sync,
              set: (on) => update((s) => setBackgroundOptions(s, { sync: on })),
            },
            { rare: true },
          ),
        ]
      : [],
  };
}

export type SlideProperties = ReturnType<typeof slideProperties>;
