// Types shared with the Rust side. They are generated from the Rust code into
// ./bindings by `cargo test` (ts-rs); change the Rust type, never these files.

import type { Slide } from "./bindings/Slide";

export type { AnswerMode } from "./bindings/AnswerMode";
export type { AppError } from "./bindings/AppError";
export type { BoardChange } from "./bindings/BoardChange";
export type { Background } from "./bindings/Background";
export type { AnswerResults } from "./bindings/AnswerResults";
export type { Button } from "./bindings/Button";
export type { Category } from "./bindings/Category";
export type { ClockView } from "./bindings/ClockView";
export type { Color } from "./bindings/Color";
export type { ColorKey } from "./bindings/ColorKey";
export type { Corners } from "./bindings/Corners";
export type { CorrectAnswer } from "./bindings/CorrectAnswer";
export type { Command } from "./bindings/Command";
export type { Controls } from "./bindings/Controls";
export type { Cue } from "./bindings/Cue";
export type { DeviceInfo } from "./bindings/DeviceInfo";
export type { EditorDraft } from "./bindings/EditorDraft";
export type { FileAction } from "./bindings/FileAction";
export type { FontName } from "./bindings/FontName";
export type { GameView } from "./bindings/GameView";
export type { Gradient } from "./bindings/Gradient";
export type { GradientKind } from "./bindings/GradientKind";
export type { HeadingCase } from "./bindings/HeadingCase";
export type { IconColor } from "./bindings/IconColor";
export type { ImportIssue } from "./bindings/ImportIssue";
export type { ImportNote } from "./bindings/ImportNote";
export type { LearnProgress } from "./bindings/LearnProgress";
export type { LibraryFile } from "./bindings/LibraryFile";
export type { ListSeparator } from "./bindings/ListSeparator";
export type { MediaLibrary } from "./bindings/MediaLibrary";
export type { MediaIssue } from "./bindings/MediaIssue";
export type { MediaKind } from "./bindings/MediaKind";
export type { MonitorInfo } from "./bindings/MonitorInfo";
export type { NowPlaying } from "./bindings/NowPlaying";
export type { Pad } from "./bindings/Pad";
export type { PadClick } from "./bindings/PadClick";
export type { PadColor } from "./bindings/PadColor";
export type { PadSource } from "./bindings/PadSource";
export type { Phase } from "./bindings/Phase";
export type { Place } from "./bindings/Place";
export type { PlayerCommand } from "./bindings/PlayerCommand";
export type { PlayerView } from "./bindings/PlayerView";
export type { Plays } from "./bindings/Plays";
export type { TeamView } from "./bindings/TeamView";
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
export type { SheetExport } from "./bindings/SheetExport";
export type { SheetImport } from "./bindings/SheetImport";
export type { Slide } from "./bindings/Slide";
export type { SlideStyle } from "./bindings/SlideStyle";
export type { SoundBoard } from "./bindings/SoundBoard";
export type { SoundClip } from "./bindings/SoundClip";
export type { SoundStart } from "./bindings/SoundStart";
export type { Span } from "./bindings/Span";
export type { TextAlign } from "./bindings/TextAlign";
export type { TextSize } from "./bindings/TextSize";
export type { TextStyle } from "./bindings/TextStyle";
export type { Theme } from "./bindings/Theme";
export type { ThemeFile } from "./bindings/ThemeFile";
export type { TimerDesign } from "./bindings/TimerDesign";
export type { Transparency } from "./bindings/Transparency";
export type { ValidationError } from "./bindings/ValidationError";
export type { VideoClip } from "./bindings/VideoClip";
export type { VideoEffect } from "./bindings/VideoEffect";

export type QuestionSlide = Extract<Slide, { type: "question" }>;
/** The slides listed by a title. */
export type TitledSlide = Extract<Slide, { type: "billboard" | "endOfRound" | "final" }>;
export type SlideType = Slide["type"];

/** Buzzer number as the hardware reports it (`BuzzerId` in Rust). */
export type BuzzerId = number;
