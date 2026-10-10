// Types shared with the Rust side. They are generated from the Rust code into
// ./bindings by `cargo test` (ts-rs); change the Rust type, never these files.

import type { Slide } from "./bindings/Slide";

export type { AnswerMode } from "./bindings/AnswerMode";
export type { AppError } from "./bindings/AppError";
export type { Background } from "./bindings/Background";
export type { AnswerResults } from "./bindings/AnswerResults";
export type { Button } from "./bindings/Button";
export type { ClockView } from "./bindings/ClockView";
export type { ColorKey } from "./bindings/ColorKey";
export type { Command } from "./bindings/Command";
export type { Controls } from "./bindings/Controls";
export type { Cue } from "./bindings/Cue";
export type { DeviceInfo } from "./bindings/DeviceInfo";
export type { EditorDraft } from "./bindings/EditorDraft";
export type { FileAction } from "./bindings/FileAction";
export type { GameView } from "./bindings/GameView";
export type { LearnProgress } from "./bindings/LearnProgress";
export type { MediaIssue } from "./bindings/MediaIssue";
export type { MediaKind } from "./bindings/MediaKind";
export type { MonitorInfo } from "./bindings/MonitorInfo";
export type { Phase } from "./bindings/Phase";
export type { PlayerView } from "./bindings/PlayerView";
export type { Press } from "./bindings/Press";
export type { PressEvent } from "./bindings/PressEvent";
export type { Problem } from "./bindings/Problem";
export type { ProtocolKind } from "./bindings/ProtocolKind";
export type { Question } from "./bindings/Question";
export type { Quiz } from "./bindings/Quiz";
export type { RawReport } from "./bindings/RawReport";
export type { ReceiverInfo } from "./bindings/ReceiverInfo";
export type { ReceiverProblem } from "./bindings/ReceiverProblem";
export type { ReceiverStatus } from "./bindings/ReceiverStatus";
export type { Replay } from "./bindings/Replay";
export type { ScreenKey } from "./bindings/ScreenKey";
export type { SessionSummary } from "./bindings/SessionSummary";
export type { SettingsLost } from "./bindings/SettingsLost";
export type { Slide } from "./bindings/Slide";
export type { SoundClip } from "./bindings/SoundClip";
export type { SoundStart } from "./bindings/SoundStart";
export type { Span } from "./bindings/Span";
export type { Theme } from "./bindings/Theme";
export type { ThemeFile } from "./bindings/ThemeFile";
export type { ValidationError } from "./bindings/ValidationError";
export type { VideoClip } from "./bindings/VideoClip";

export type QuestionSlide = Extract<Slide, { type: "question" }>;
export type SlideType = Slide["type"];

/** Buzzer number as the hardware reports it (`BuzzerId` in Rust). */
export type BuzzerId = number;
