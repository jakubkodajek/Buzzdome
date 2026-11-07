// Backend codes in the quizmaster's language. The backend never sends an
// English sentence for the UI: errors, quiz problems and receiver states come
// as codes with parameters (generated types), worded here from `i18n.ts`.

import { ANSWER_LETTERS, buttonName } from "./answers";
import { slotLabel } from "./audio/library";
import { MEDIA_EXTENSIONS } from "./bindings/MediaExtensions";
import { fileName } from "./format";
import type { StringKey, Translate } from "./i18n";
import { mediaFileName } from "./media";
import type {
  AppError,
  AiConnected,
  Assistant,
  ColourClash,
  ConnectChange,
  FileAction,
  FileReason,
  ImportIssue,
  Light,
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
type WordedApart = "file" | "noSuchSlide" | "questionRestarted" | "noSuchPad" | "notInLibrary" | "learnConflict" | "buzzerColours" | "quizInvalid" | "journalBroken" | "lightsRefused";

const ERROR_KEYS: Record<Exclude<AppError["code"], WordedApart>, StringKey> = {
  noShow: "noShow",
  showRunning: "errShowRunning",
  mediaDuringShow: "errMediaDuringShow",
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
  autoStepHeld: "errAutoStepHeld",
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
  quizFileGone: "errQuizFileGone",
  quizFileUnreachable: "errQuizFileUnreachable",
  draftChanged: "errDraftChanged",
  draftNotRead: "errDraftNotRead",
  notASpreadsheet: "errNotASpreadsheet",
  droppedFileTooLarge: "mediaDropTooLarge",
  spreadsheetTooLarge: "errSpreadsheetTooLarge",
  spreadsheetTooComplex: "errSpreadsheetTooComplex",
  spreadsheetReaderFailed: "errSpreadsheetReaderFailed",
  noQuestionColumn: "errNoQuestionColumn",
  aiAppMissing: "errAiAppMissing",
  aiConfigUnreadable: "errAiConfigUnreadable",
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
  voiceInstalling: "errVoiceInstalling",
  diskBusy: "errDiskBusy",
  noDataFolder: "errNoDataFolder",
  system: "errSystem",
  libraryDownload: "errLibraryDownload",
  libraryRedirected: "errLibraryRedirected",
  libraryNotRead: "errLibraryNotRead",
  libraryFormatChanged: "errLibraryFormatChanged",
  libraryTooSmall: "errLibraryTooSmall",
  voiceDownload: "errVoiceDownload",
  voiceDamaged: "errVoiceDamaged",
  voiceDownloadStopped: "errVoiceDownloadStopped",
  voiceDownloadCancelled: "errVoiceDownloadCancelled",
  voiceMissing: "errVoiceMissing",
  voiceFailed: "errVoiceFailed",
  voiceDisk: "errVoiceDisk",
};

const FILE_ACTION_KEYS: Record<FileAction, StringKey> = {
  read: "errFileRead",
  write: "errFileWrite",
  createFolder: "errFileCreateFolder",
  move: "errFileMove",
  delete: "errFileDelete",
};

const FILE_REASON_KEYS: Record<Exclude<FileReason, "other">, StringKey> = {
  notFound: "fileNotFound",
  notAFolder: "fileNotAFolder",
  noAccess: "fileNoAccess",
  inUse: "fileInUse",
  diskFull: "fileDiskFull",
  readOnly: "fileReadOnly",
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
 * A message for the quizmaster: what happened, in their words, and apart
 * from it the system's own words or the file concerned (`detail`), which
 * help find the cause but are not theirs.
 */
export interface Notice {
  text: string;
  detail?: string;
}

/** The system's words without the error number Rust adds to them ("(os error 2)"). */
export function systemWords(detail: string): string {
  return detail.replace(/\s*\(os error -?\d+\)\s*$/, "").trim();
}

/**
 * A failed backend call for the quizmaster. Anything that is not an
 * `AppError` (a failure inside Tauri's own IPC, a script error) is shown as it is.
 */
export function errorNotice(t: Translate, error: unknown): Notice {
  if (!isAppError(error)) return { text: thrownText(error) };
  if (error.code === "file") {
    const text = t(FILE_ACTION_KEYS[error.action], { name: fileName(error.path) });
    if (error.reason === "other") return { text, detail: `${error.path}: ${systemWords(error.detail)}` };
    return { text: `${text} ${t(FILE_REASON_KEYS[error.reason])}`, detail: error.path };
  }
  const text = errorWords(t, error);
  return "detail" in error ? { text, detail: systemWords(error.detail) } : { text };
}

/** A failed backend call in one line, for a message that has no room for its detail apart. */
export function errorText(t: Translate, error: unknown): string {
  const { text, detail } = errorNotice(t, error);
  return detail ? `${text} (${detail})` : text;
}

/** The quizmaster's words for `error`, without its detail. */
function errorWords(t: Translate, error: Exclude<AppError, { code: "file" }>): string {
  switch (error.code) {
    case "noSuchSlide":
      return t("errNoSuchSlide", { slide: error.slide + 1 });
    case "questionRestarted":
      return t("errQuestionRestarted", { slide: error.slide + 1 });
    case "noSuchPad":
      return t("errNoSuchPad", { pad: error.pad + 1 });
    case "notInLibrary":
      return t("errNotInLibrary", { reference: mediaFileName(error.reference) });
    case "learnConflict":
      return t("errLearnConflict", { buzzer: error.buzzer, button: buttonName(t, error.button) });
    case "buzzerColours":
      return colourClashText(t, error.clash);
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
type ProblemApart = "media" | "categoryMedia" | "soundMedia" | "welcomeMedia" | "volume" | "partEmpty" | "replayNotOnQuestion" | "answerEmpty";

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
  valueUnread: "problemValueUnread",
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
  noSlides: "problemNoSlides",
  questionTextEmpty: "problemQuestionTextEmpty",
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
    case "answerEmpty":
      return t("problemAnswerEmpty", { letter: ANSWER_LETTERS[problem.answer] ?? String(problem.answer + 1) });
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
  const text = t(RECEIVER_PROBLEM_KEYS[problem], { detail: systemWords(detail) });
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

/** `lead`, what was lost, with what became of the file the app keeps, and why it was not read apart. */
export function settingsLostNotice(t: Translate, lead: string, lost: SettingsLost): Notice {
  const file = lost.keptAs === null ? t("settingsLeftUnread") : t("settingsKeptAs", { path: lost.keptAs });
  return { text: `${lead} ${file}`, detail: systemWords(lost.detail) };
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

const LIGHT_KEYS: Record<Light, StringKey> = {
  off: "lightOff",
  ready: "lightReady",
  answered: "lightAnswered",
  waiting: "lightWaiting",
  locked: "lightLocked",
  first: "lightFirst",
  correct: "lightCorrect",
  wrong: "lightWrong",
};

/** A light of the buzzers, as the Receiver tab names it. */
export function lightText(t: Translate, light: Light): string {
  return t(LIGHT_KEYS[light]);
}

/** Why buzzer colours were refused. */
export function colourClashText(t: Translate, clash: ColourClash): string {
  const ready = lightText(t, "ready");
  return clash.kind === "readyDark" ? t("coloursReadyDark", { ready }) : t("coloursReadyLike", { ready, light: lightText(t, clash.light) });
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
    case "lookLosesFixtures":
      return t("lightsProblemLookLosesFixtures", { look: error.look });
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
  return t(OUTPUT_ERROR_KEYS[error.problem], params("detail" in error ? { ...error, detail: systemWords(error.detail) } : error));
}

const CONNECT_CHANGE_KEYS: Record<ConnectChange, StringKey> = {
  added: "aiConnected",
  updated: "aiReconnected",
  unchanged: "aiAlreadyConnected",
};

/**
 * What connecting an AI app changed, where it works, and what the quizmaster does next on `platform`
 * (`std::env::consts::OS`).
 */
export function aiConnectionText(t: Translate, connected: AiConnected, platform: string): string {
  const said = t(CONNECT_CHANGE_KEYS[connected.change], { app: connected.name, folder: connected.folder });
  // A running app keeps the folder it started with, also when its entry was already there.
  return `${said} ${aiRestartText(t, connected.assistant, platform)}`;
}

/** How `assistant` starts using the server once it is connected, on `platform` (`std::env::consts::OS`). */
export function aiRestartText(t: Translate, assistant: Assistant, platform: string): string {
  switch (assistant) {
    case "claudeDesktop":
      return t(platform === "macos" ? "aiRestartClaudeMacos" : platform === "windows" ? "aiRestartClaudeWindows" : "aiRestartClaudeLinux");
    case "antigravity":
      return t("aiRestartAntigravity");
    case "codex":
      return t(platform === "macos" ? "aiRestartCodexMacos" : "aiRestartCodexOther");
  }
}
