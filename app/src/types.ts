// Types shared with the Rust side. They are generated from the Rust code into
// ./bindings by `cargo test` (ts-rs); change the Rust type, never these files.

import type { Slide } from "./bindings/Slide";

export type { AnswerMode } from "./bindings/AnswerMode";
export type { Button } from "./bindings/Button";
export type { ClockView } from "./bindings/ClockView";
export type { Command } from "./bindings/Command";
export type { Cue } from "./bindings/Cue";
export type { DeviceInfo } from "./bindings/DeviceInfo";
export type { EditorDraft } from "./bindings/EditorDraft";
export type { GameView } from "./bindings/GameView";
export type { LearnProgress } from "./bindings/LearnProgress";
export type { MonitorInfo } from "./bindings/MonitorInfo";
export type { Phase } from "./bindings/Phase";
export type { PlayerView } from "./bindings/PlayerView";
export type { Press } from "./bindings/Press";
export type { PressEvent } from "./bindings/PressEvent";
export type { Question } from "./bindings/Question";
export type { Quiz } from "./bindings/Quiz";
export type { RawReport } from "./bindings/RawReport";
export type { ReceiverInfo } from "./bindings/ReceiverInfo";
export type { ReceiverStatus } from "./bindings/ReceiverStatus";
export type { SessionSummary } from "./bindings/SessionSummary";
export type { Slide } from "./bindings/Slide";
export type { SoundClip } from "./bindings/SoundClip";
export type { SoundStart } from "./bindings/SoundStart";
export type { ValidationError } from "./bindings/ValidationError";
export type { VideoClip } from "./bindings/VideoClip";

export type QuestionSlide = Extract<Slide, { type: "question" }>;
export type SlideType = Slide["type"];

/** Buzzer number as the hardware reports it (`BuzzerId` in Rust). */
export type BuzzerId = number;
