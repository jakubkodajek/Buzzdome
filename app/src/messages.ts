// Backend codes in the quizmaster's language. The backend never sends an
// English sentence for the UI: errors, quiz problems and receiver states come
// as codes with parameters (generated types), worded here from `i18n.ts`.

import { buttonLabel } from "./answers";
import { MEDIA_EXTENSIONS } from "./bindings/MediaExtensions";
import type { StringKey, Translate } from "./i18n";
import { mediaFileName } from "./media";
import type { AppError, FileAction, MediaIssue, MediaKind, Problem, ProtocolKind, ReceiverProblem } from "./types";

type Params = Record<string, string | number>;

/** The string and number fields of a code, as text parameters. */
function params(fields: object): Params {
  return Object.fromEntries(
    Object.entries(fields).filter((entry): entry is [string, string | number] => ["string", "number"].includes(typeof entry[1])),
  );
}

/** Codes worded with their own parameters, see `errorText`. */
type WordedApart = "file" | "noSuchSlide" | "noSuchMonitor" | "noSuchPad" | "notInLibrary" | "learnConflict" | "quizInvalid";

const ERROR_KEYS: Record<Exclude<AppError["code"], WordedApart>, StringKey> = {
  noShow: "noShow",
  showRunning: "errShowRunning",
  noShowToResume: "errNoShowToResume",
  notNow: "errNotNow",
  unknownBuzzer: "errUnknownBuzzer",
  duplicateBuzzer: "errDuplicateBuzzer",
  tooManyTeams: "errTooManyTeams",
  showNotSaved: "errShowNotSaved",
  showNotRead: "errShowNotRead",
  notAQuizFile: "errNotAQuizFile",
  missingQuizJson: "errMissingQuizJson",
  badQuizJson: "errBadQuizJson",
  newerSchema: "errNewerSchema",
  tooLarge: "errTooLarge",
  tooManyFiles: "errTooManyFiles",
  noDraft: "errNoDraft",
  notAMediaFile: "errNotAMediaFile",
  usbUnavailable: "errUsbUnavailable",
  usbListFailed: "errUsbListFailed",
  receiverNotConnected: "errReceiverNotConnected",
  learnNotStarted: "errLearnNotStarted",
  learnTooFewPresses: "errLearnTooFewPresses",
  learnInconsistent: "errLearnInconsistent",
  system: "errSystem",
};

const FILE_ACTION_KEYS: Record<FileAction, StringKey> = {
  read: "errFileRead",
  write: "errFileWrite",
  createFolder: "errFileCreateFolder",
  move: "errFileMove",
  delete: "errFileDelete",
};

function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "code" in value && typeof value.code === "string";
}

/**
 * A failed backend call for the quizmaster. Anything that is not an
 * `AppError` (a failure inside Tauri's own IPC, a script error) is shown as it is.
 */
export function errorText(t: Translate, error: unknown): string {
  if (!isAppError(error)) return error instanceof Error ? error.message : String(error);
  switch (error.code) {
    case "file":
      return t(FILE_ACTION_KEYS[error.action], params(error));
    case "noSuchSlide":
      return t("errNoSuchSlide", { slide: error.slide + 1 });
    case "noSuchMonitor":
      return t("errNoSuchMonitor", { n: error.index + 1 });
    case "noSuchPad":
      return t("errNoSuchPad", { pad: error.pad + 1 });
    case "notInLibrary":
      return t("errNotInLibrary", { reference: mediaFileName(error.reference) });
    case "learnConflict":
      return t("errLearnConflict", { buzzer: error.buzzer, button: buttonLabel(error.button) });
    case "quizInvalid": {
      const first = error.problems[0];
      return t("errQuizInvalid", { n: error.problems.length, first: first ? problemText(t, first.problem) : "" });
    }
    default:
      return t(ERROR_KEYS[error.code], params(error));
  }
}

/** Problems worded with their own parameters, see `problemText`. */
type ProblemApart = "media" | "volume" | "partEmpty" | "replayNotOnQuestion";

const PROBLEM_KEYS: Record<Exclude<Problem["code"], ProblemApart>, StringKey> = {
  duplicateSlideId: "problemDuplicateSlideId",
  pictureAndVideo: "problemPictureAndVideo",
  replayDelayTooLong: "problemReplayDelayTooLong",
  soundGainOutOfRange: "problemSoundGainOutOfRange",
  videoSpeedOutOfRange: "problemVideoSpeedOutOfRange",
  introPauseTooLong: "problemIntroPauseTooLong",
  withClockNotOnQuestion: "problemWithClockNotOnQuestion",
  effectNotOnQuestion: "problemEffectNotOnQuestion",
  mediaOnFinal: "problemMediaOnFinal",
  tooManyAnswers: "problemTooManyAnswers",
  tooFewAnswers: "problemTooFewAnswers",
  noCorrectAnswer: "problemNoCorrectAnswer",
  correctOutOfRange: "problemCorrectOutOfRange",
  pointsOutOfRange: "problemPointsOutOfRange",
  penaltyOutOfRange: "problemPenaltyOutOfRange",
  timeLimitTooLong: "problemTimeLimitTooLong",
};

const MEDIA_ISSUE_KEYS: Record<MediaIssue["type"], StringKey> = {
  notInMediaFolder: "mediaNotInMediaFolder",
  badName: "mediaBadName",
  wrongType: "mediaWrongType",
  missing: "mediaMissing",
};

const MEDIA_KIND_KEYS: Record<MediaKind, StringKey> = {
  image: "mediaKindImage",
  audio: "mediaKindAudio",
  video: "mediaKindVideo",
};

/** A problem that keeps a quiz from being played. */
export function problemText(t: Translate, problem: Problem): string {
  switch (problem.code) {
    case "media":
      return t("problemMedia", { file: problem.file, issue: mediaIssueText(t, problem.issue) });
    // Rules a sound and a video share, said of the one that breaks them.
    case "volume":
      return t(problem.media === "audio" ? "problemSoundVolume" : "problemVideoVolume");
    case "partEmpty":
      return t(problem.media === "audio" ? "problemSoundPartEmpty" : "problemVideoPartEmpty");
    case "replayNotOnQuestion":
      return t(problem.media === "audio" ? "problemSoundReplayNotOnQuestion" : "problemVideoReplayNotOnQuestion");
    default:
      return t(PROBLEM_KEYS[problem.code], params(problem));
  }
}

function mediaIssueText(t: Translate, issue: MediaIssue): string {
  if (issue.type !== "wrongType") return t(MEDIA_ISSUE_KEYS[issue.type]);
  const kinds = issue.expected.map((kind) => t(MEDIA_KIND_KEYS[kind], { extensions: MEDIA_EXTENSIONS[kind].join(", ") }));
  return t("mediaWrongType", { kinds: kinds.join(` ${t("or")} `) });
}

const RECEIVER_PROBLEM_KEYS: Record<ReceiverProblem, StringKey> = {
  cannotList: "receiverCannotList",
  cannotOpen: "receiverCannotOpen",
  connectionLost: "receiverConnectionLost",
  writeFailed: "receiverWriteFailed",
};

/**
 * Why the receiver cannot be used. A receiver that cannot be opened gets
 * what to check on `platform` (`std::env::consts::OS`).
 */
export function receiverProblemText(t: Translate, problem: ReceiverProblem, detail: string, platform: string): string {
  const text = t(RECEIVER_PROBLEM_KEYS[problem], { detail });
  if (problem !== "cannotOpen") return text;
  const hint = platform === "linux" ? "receiverOpenHintLinux" : platform === "macos" ? "receiverOpenHintMacos" : "receiverOpenHintOther";
  return `${text} ${t(hint)}`;
}

/** Which protocol reads the receiver. */
export function protocolText(t: Translate, protocol: ProtocolKind): string {
  return protocol.type === "learned" ? t("protocolLearned", { buttons: protocol.buttons }) : t("protocolRawOnly");
}
