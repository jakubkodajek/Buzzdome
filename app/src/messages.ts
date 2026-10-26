// Backend codes in the quizmaster's language. The backend never sends an
// English sentence for the UI: errors, quiz problems and receiver states come
// as codes with parameters (generated types), worded here from `i18n.ts`.

import { buttonLabel } from "./answers";
import { slotLabel } from "./audio/library";
import { MEDIA_EXTENSIONS } from "./bindings/MediaExtensions";
import type { StringKey, Translate } from "./i18n";
import { mediaFileName } from "./media";
import type {
  AppError,
  ClaudeConnected,
  ClaudeConnection,
  FileAction,
  ImportIssue,
  MediaIssue,
  MediaKind,
  OutputError,
  Part,
  Place,
  Problem,
  ProtocolKind,
  ReceiverProblem,
  ReceiverStatus,
  RemoteAction,
  RemoteKey,
  SettingsLost,
  SetupError,
  WelcomeFile,
} from "./types";

type Params = Record<string, string | number>;

/** The string and number fields of a code, as text parameters. */
function params(fields: object): Params {
  return Object.fromEntries(
    Object.entries(fields).filter((entry): entry is [string, string | number] => ["string", "number"].includes(typeof entry[1])),
  );
}

/** Codes worded with their own parameters, see `errorText`. */
type WordedApart = "file" | "noSuchSlide" | "noSuchPad" | "notInLibrary" | "learnConflict" | "quizInvalid" | "journalBroken" | "lightsRefused";

const ERROR_KEYS: Record<Exclude<AppError["code"], WordedApart>, StringKey> = {
  noShow: "noShow",
  showRunning: "errShowRunning",
  showStarting: "errShowStarting",
  noShowToResume: "errNoShowToResume",
  notNow: "errNotNow",
  notFromProjector: "errNotFromProjector",
  unknownBuzzer: "errUnknownBuzzer",
  duplicateBuzzer: "errDuplicateBuzzer",
  tooManyTeams: "errTooManyTeams",
  showNotSaved: "errShowNotSaved",
  showStopped: "errShowStopped",
  showBusy: "errShowBusy",
  saveHangs: "errSaveHangs",
  showNotRead: "errShowNotRead",
  journalDamaged: "errJournalDamaged",
  fileLeftUnread: "errFileLeftUnread",
  rulesChanged: "errRulesChanged",
  mediaFolderRefused: "errMediaFolderRefused",
  notAQuizFile: "errNotAQuizFile",
  missingQuizJson: "errMissingQuizJson",
  badQuizJson: "errBadQuizJson",
  newerSchema: "errNewerSchema",
  tooLarge: "errTooLarge",
  quizJsonTooLarge: "errQuizJsonTooLarge",
  tooManyFiles: "errTooManyFiles",
  tooManyEndRecords: "errTooManyEndRecords",
  noDraft: "errNoDraft",
  notRecent: "errNotRecent",
  draftChanged: "errDraftChanged",
  draftNotRead: "errDraftNotRead",
  notASpreadsheet: "errNotASpreadsheet",
  droppedFileTooLarge: "mediaDropTooLarge",
  spreadsheetTooLarge: "errSpreadsheetTooLarge",
  spreadsheetTooComplex: "errSpreadsheetTooComplex",
  spreadsheetReaderFailed: "errSpreadsheetReaderFailed",
  noQuestionColumn: "errNoQuestionColumn",
  claudeDesktopMissing: "errClaudeDesktopMissing",
  claudeConfigUnreadable: "errClaudeConfigUnreadable",
  aiFolderNotAllowed: "errAiFolderNotAllowed",
  noDocumentsFolder: "errNoDocumentsFolder",
  notAMediaFile: "errNotAMediaFile",
  usbUnavailable: "errUsbUnavailable",
  usbListFailed: "errUsbListFailed",
  receiverNotConnected: "errReceiverNotConnected",
  receiverNotLearnable: "errReceiverNotLearnable",
  learnNotStarted: "errLearnNotStarted",
  learnTooFewPresses: "errLearnTooFewPresses",
  learnInconsistent: "errLearnInconsistent",
  noUpdate: "errNoUpdate",
  updateFailed: "errUpdateFailed",
  updateInstalling: "errUpdateInstalling",
  noDataFolder: "errNoDataFolder",
  system: "errSystem",
  libraryDownload: "errLibraryDownload",
  libraryRedirected: "errLibraryRedirected",
  libraryNotRead: "errLibraryNotRead",
  libraryFormatChanged: "errLibraryFormatChanged",
  libraryTooSmall: "errLibraryTooSmall",
};

const FILE_ACTION_KEYS: Record<FileAction, StringKey> = {
  read: "errFileRead",
  write: "errFileWrite",
  createFolder: "errFileCreateFolder",
  move: "errFileMove",
  delete: "errFileDelete",
};

/** Whether `value` is a backend error (an `AppError`), not one of Tauri's own or a script's. */
export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "code" in value && typeof value.code === "string";
}

/**
 * Anything thrown, as text: an error's message, a string itself, any other
 * value as JSON where it has one. Never throws itself, not even on a value
 * that refers to itself.
 */
export function thrownText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  try {
    // `undefined` and a function have no JSON.
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

/**
 * A failed backend call for the quizmaster. Anything that is not an
 * `AppError` (a failure inside Tauri's own IPC, a script error) is shown as it is.
 */
export function errorText(t: Translate, error: unknown): string {
  if (!isAppError(error)) return thrownText(error);
  switch (error.code) {
    case "file":
      return t(FILE_ACTION_KEYS[error.action], params(error));
    case "noSuchSlide":
      return t("errNoSuchSlide", { slide: error.slide + 1 });
    case "noSuchPad":
      return t("errNoSuchPad", { pad: error.pad + 1 });
    case "notInLibrary":
      return t("errNotInLibrary", { reference: mediaFileName(error.reference) });
    case "learnConflict":
      return t("errLearnConflict", { buzzer: error.buzzer, button: buttonLabel(error.button) });
    case "journalBroken":
      return t("errJournalBroken", { resume: t("resume") });
    case "lightsRefused":
      return t("errLightsRefused", { problem: setupErrorText(t, error.refused) });
    case "quizInvalid": {
      const first = error.problems[0];
      return t("errQuizInvalid", { n: error.problems.length, first: first ? problemText(t, first.problem) : "" });
    }
    default:
      return t(ERROR_KEYS[error.code], params(error));
  }
}

/** Problems worded with their own parameters, see `problemText`. */
type ProblemApart = "media" | "categoryMedia" | "soundMedia" | "welcomeMedia" | "volume" | "partEmpty" | "replayNotOnQuestion";

const PROBLEM_KEYS: Record<Exclude<Problem["code"], ProblemApart>, StringKey> = {
  duplicateSlideId: "problemDuplicateSlideId",
  pictureAndVideo: "problemPictureAndVideo",
  replayDelayTooLong: "problemReplayDelayTooLong",
  soundGainOutOfRange: "problemSoundGainOutOfRange",
  videoSpeedOutOfRange: "problemVideoSpeedOutOfRange",
  introPauseTooLong: "problemIntroPauseTooLong",
  withClockNotOnQuestion: "problemWithClockNotOnQuestion",
  effectNotOnQuestion: "problemEffectNotOnQuestion",
  mediaOnRanking: "problemMediaOnRanking",
  styleUnread: "problemStyleUnread",
  categoryStyleUnread: "problemCategoryStyleUnread",
  themeUnread: "problemThemeUnread",
  unknownCategory: "problemUnknownCategory",
  duplicateCategoryId: "problemDuplicateCategoryId",
  categoryIdInvalid: "problemCategoryIdInvalid",
  categoryUnnamed: "problemCategoryUnnamed",
  categoryNameTooLong: "problemCategoryNameTooLong",
  categoryNameTaken: "problemCategoryNameTaken",
  mediaNameClash: "problemMediaNameClash",
  tooManyAnswers: "problemTooManyAnswers",
  tooFewAnswers: "problemTooFewAnswers",
  noCorrectAnswer: "problemNoCorrectAnswer",
  correctOutOfRange: "problemCorrectOutOfRange",
  correctTwice: "problemCorrectTwice",
  shareOutOfRange: "problemShareOutOfRange",
  shareNotOnEveryone: "problemShareNotOnEveryone",
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

/** What each file of the welcome screen is called. */
const WELCOME_FILE_LABEL: Record<WelcomeFile, StringKey> = {
  logo: "welcomeLogo",
  intro: "welcomeIntro",
};

/** A problem that keeps a quiz from being played, or that opening its file left out. */
export function problemText(t: Translate, problem: Problem): string {
  switch (problem.code) {
    case "media":
      return t("problemMedia", { file: problem.file, issue: mediaIssueText(t, problem.issue) });
    case "categoryMedia":
      return t("problemCategoryMedia", { name: problem.name, file: problem.file, issue: mediaIssueText(t, problem.issue) });
    case "soundMedia":
      return t("problemSoundMedia", { moment: slotLabel(t, problem.slot), file: problem.file, issue: mediaIssueText(t, problem.issue) });
    case "welcomeMedia":
      return t("problemWelcomeMedia", { place: t(WELCOME_FILE_LABEL[problem.place]), file: problem.file, issue: mediaIssueText(t, problem.issue) });
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

/** A USB number as four hex digits, as the system lists devices: "10c4". */
export const hex4 = (n: number) => n.toString(16).padStart(4, "0");
/** A USB device's vendor and product numbers: "10c4:1819". */
export const vidPid = (d: { vendorId: number; productId: number }) => `${hex4(d.vendorId)}:${hex4(d.productId)}`;

/** The receiver's state, as the Receiver tab's status line says it (`platform` as for `receiverProblemText`). */
export function receiverStatusText(t: Translate, status: ReceiverStatus, platform: string): string {
  switch (status.kind) {
    case "connected":
      return t(awaitsAnswer(status.protocol) ? "receiverNotReady" : "receiverConnected", {
        name: status.device.product ?? "HID",
        id: vidPid(status.device),
        protocol: protocolText(t, status.protocol),
      });
    case "looking":
      return t("receiverLooking");
    case "searching":
      return t("receiverSearching", { id: vidPid(status.selector) });
    case "error":
      return receiverProblemText(t, status.problem, status.detail, platform);
    case "off":
      return t("receiverOff");
  }
}

/** Why a file the app keeps was not read, and what became of the file. */
export function settingsLostText(t: Translate, lost: SettingsLost): string {
  const file = lost.keptAs === null ? t("settingsLeftUnread") : t("settingsKeptAs", { path: lost.keptAs });
  return `${lost.detail} ${file}`;
}

/** Which protocol reads the receiver. */
export function protocolText(t: Translate, protocol: ProtocolKind): string {
  switch (protocol.type) {
    case "rawOnly":
      return t("protocolRawOnly");
    case "learned":
      return t("protocolLearned", { buttons: protocol.buttons });
    case "enjoy303s":
      return t("protocolEnjoy303s");
  }
}

const REMOTE_KEY_KEYS: Record<RemoteKey, StringKey> = {
  back: "remoteKeyBack",
  forward: "remoteKeyForward",
  power: "remoteKeyPower",
  grid: "remoteKeyGrid",
  target: "remoteKeyTarget",
  playPause: "remoteKeyPlayPause",
  centre: "remoteKeyCentre",
  question: "remoteKeyQuestion",
  trophy: "remoteKeyTrophy",
  colourBars: "remoteKeyColourBars",
  cross: "remoteKeyCross",
  tick: "remoteKeyTick",
};

/** A key of the quizmaster's remote, by what is printed on it. */
export function remoteKeyText(t: Translate, key: RemoteKey): string {
  return t(REMOTE_KEY_KEYS[key]);
}

const REMOTE_ACTION_KEYS: Record<RemoteAction, StringKey> = {
  nothing: "remoteNothing",
  next: "remoteNext",
  previous: "remotePrevious",
  clock: "remoteClock",
  closeQuestion: "remoteCloseQuestion",
  judgeRight: "remoteJudgeRight",
  judgeWrong: "remoteJudgeWrong",
  leaderboard: "remoteLeaderboard",
  playVideo: "remotePlayVideo",
  projector: "remoteProjector",
};

/** Every action a key of the remote can do, in the order the quizmaster picks from. */
export const REMOTE_ACTIONS = Object.keys(REMOTE_ACTION_KEYS) as RemoteAction[];

export function remoteActionText(t: Translate, action: RemoteAction): string {
  return t(REMOTE_ACTION_KEYS[action]);
}

/** Whether the receiver has yet to answer its set-up: shown as still being looked for, not as connected. */
export function awaitsAnswer(protocol: ProtocolKind): boolean {
  return protocol.type === "enjoy303s" && !protocol.answered;
}


const IMPORT_ISSUE_KEYS: Record<ImportIssue["code"], StringKey> = {
  noQuestionColumn: "importNoQuestionColumn",
  unknownColumn: "importUnknownColumn",
  unsupportedColumn: "importUnsupportedColumn",
  duplicateColumn: "importDuplicateColumn",
  secondLanguage: "importSecondLanguage",
  linkedFile: "importLinkedFile",
  slideTypeNotSupported: "importSlideTypeNotSupported",
  testQuestion: "importTestQuestion",
  qualifierNotSupported: "importQualifierNotSupported",
  notEveryone: "importNotEveryone",
  answersLeftOut: "importAnswersLeftOut",
  unknownCategory: "importUnknownCategory",
  pictureAndVideo: "importPictureAndVideo",
  twoBackgrounds: "importTwoBackgrounds",
  mediaOnRanking: "importMediaOnRanking",
  wrongMediaType: "importWrongMediaType",
  badValue: "importBadValue",
  unknownSetting: "importUnknownSetting",
  badSetting: "importBadSetting",
  settingNotUsed: "importSettingNotUsed",
  fileNotFound: "importFileNotFound",
  fileOutsideSheetFolder: "importFileOutsideSheetFolder",
  networkPath: "importNetworkPath",
};

/** Something a spreadsheet import could not take over as the sheet has it. */
export function importIssueText(t: Translate, issue: ImportIssue): string {
  return t(IMPORT_ISSUE_KEYS[issue.code], params(issue));
}

/** Rows of an import note listed before "and n more". */
const PLACES_SHOWN = 8;

/** The rows an import note came up in, by sheet: "rows 3, 5; sheet “Round 2”, row 4". */
export function placesText(t: Translate, places: Place[]): string {
  const sheets = new Map<string, number[]>();
  for (const { sheet, row } of places.slice(0, PLACES_SHOWN)) sheets.set(sheet, [...(sheets.get(sheet) ?? []), row]);
  const parts = [...sheets].map(([sheet, rows]) => {
    const worded = t(rows.length === 1 ? "importRow" : "importRows", { rows: rows.join(", ") });
    return sheet ? t("importSheetRows", { sheet, rows: worded }) : worded;
  });
  const more = places.length - PLACES_SHOWN;
  return more > 0 ? `${parts.join("; ")} ${t("importMore", { n: more })}` : parts.join("; ");
}

const PART_KEYS: Record<Part, StringKey> = {
  fixture: "lightsPartFixture",
  channel: "lightsPartChannel",
  look: "lightsPartLook",
  color: "lightsPartColor",
  flash: "lightsPartFlash",
  output: "lightsPartOutput",
  message: "lightsPartMessage",
  master: "lightsPartMaster",
};

/** Why a lights setup was refused. */
export function setupErrorText(t: Translate, error: SetupError): string {
  switch (error.problem) {
    case "tooMany":
      return t("lightsProblemTooMany", { what: t(PART_KEYS[error.what]) });
    case "outOfRange":
      return t("lightsProblemOutOfRange", { what: t(PART_KEYS[error.what]) });
    case "sameId":
      return t("lightsProblemSameId", { what: t(PART_KEYS[error.what]) });
    case "missing":
      return t("lightsProblemMissing");
    case "pastUniverse":
      return t("lightsProblemPastUniverse", { name: error.name });
    case "notIpv4":
      return t("lightsProblemNotIpv4", { host: error.host });
  }
}

const OUTPUT_ERROR_KEYS: Record<OutputError["problem"], StringKey> = {
  notAvailable: "lightsOutputNotAvailable",
  notFound: "lightsOutputNotFound",
  unknownHost: "lightsOutputUnknownHost",
  sendFailed: "lightsOutputSendFailed",
  sentPart: "lightsOutputSentPart",
  notSupported: "lightsOutputNotSupported",
};

/** Why a lights output or message target does not work. */
export function outputErrorText(t: Translate, error: OutputError): string {
  return t(OUTPUT_ERROR_KEYS[error.problem], params(error));
}

const CLAUDE_CONNECTION_KEYS: Record<ClaudeConnection, StringKey> = {
  added: "claudeConnected",
  updated: "claudeReconnected",
  unchanged: "claudeAlreadyConnected",
};

/** What connecting Claude Desktop changed, where it may work, and what the quizmaster does next. */
export function claudeConnectionText(t: Translate, connected: ClaudeConnected): string {
  return t(CLAUDE_CONNECTION_KEYS[connected.connection], { folder: connected.folders.join(", ") });
}
