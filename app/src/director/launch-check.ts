// The check before a show (spec SF-2): while the show waits before its first
// slide, on the welcome screen or the registration, the Director lists what
// could spoil it, each part with a way to put it right. None of them holds
// the show back.

import { useCallback, useMemo, useState } from "react";
import { diskSize } from "../format";
import type { Translate } from "../i18n";
import { awaitsAnswer, receiverStatusText } from "../messages";
import { readPreference, writePreference } from "../preferences";
import type { GameView, MonitorInfo, ProjectorWindow, ReceiverInfo } from "../types";

export type CheckPart = "receiver" | "buzzers" | "projector" | "sound" | "disk";

/**
 * How a part stands: ready; a warning (the show plays, though not as
 * planned); a problem (the show may fail); or for the quizmaster to try.
 */
export type CheckState = "ok" | "warn" | "problem" | "todo";

/** What the check offers to put a part right. */
export type CheckFix = "receiverTab" | "teamsTab" | "mediaTab" | "openProjector" | "placeProjector" | "playSound" | "soundHeard";

export interface CheckItem {
  part: CheckPart;
  state: CheckState;
  text: string;
  fixes: CheckFix[];
}

/** How far the quizmaster got with the test sound: played it, and heard it in the hall. */
export type SoundTry = "notPlayed" | "played" | "heard";

/** Why the show's sounds play at no volume: the media player's volume, or the quiz's effects. */
export type Muted = "player" | "quiz" | null;

export interface LaunchFacts {
  /** Null until loaded. */
  receiver: ReceiverInfo | null;
  view: GameView;
  /** Where the projector window is: null while it is closed, undefined until loaded. */
  projector: ProjectorWindow | null | undefined;
  muted: Muted;
  sound: SoundTry;
  /** Free bytes on the disk the show is saved to: null when the system does not say, undefined until loaded. */
  freeDiskBytes: number | null | undefined;
}

const MB = 1024 ** 2;
/** Below this the show's journal may soon not be written. */
export const DISK_PROBLEM_BYTES = 100 * MB;
/**
 * Below this a warning: the journal takes little, but the system and the
 * app's other files, such as an unpacked quiz, need room too.
 */
export const DISK_WARN_BYTES = 1024 * MB;
/** The most team names a part lists; the rest are counted. */
const LISTED_TEAMS = 5;

/** Every part of the check, in the order the Director lists them. A part the system says nothing about is left out. */
export function launchCheck(t: Translate, facts: LaunchFacts): CheckItem[] {
  const parts = [
    receiverCheck(t, facts.receiver),
    buzzersCheck(t, facts.view),
    projectorCheck(t, facts.projector),
    soundCheck(t, facts.muted, facts.sound),
    diskCheck(t, facts.freeDiskBytes),
  ];
  return parts.filter((item) => item !== null);
}

function checking(t: Translate, part: CheckPart): CheckItem {
  return { part, state: "todo", text: t("launchChecking"), fixes: [] };
}

/** Without a receiver the keyboard and the on-screen buzzers still play, so a show can go on that way. */
function receiverCheck(t: Translate, info: ReceiverInfo | null): CheckItem {
  if (!info) return checking(t, "receiver");
  const fixes: CheckFix[] = ["receiverTab"];
  if (!info.status) return { part: "receiver", state: "warn", text: t("receiverUnavailable"), fixes };
  const text = receiverStatusText(t, info.status, info.platform);
  const ready = info.status.kind === "connected" && !awaitsAnswer(info.status.protocol);
  if (ready) return { part: "receiver", state: "ok", text, fixes: [] };
  const state = info.status.kind === "error" ? "problem" : "warn";
  return { part: "receiver", state, text: `${text} ${t("launchKeyboardOnly")}`, fixes };
}

/** Every prepared team should have a buzzer by the time the show begins; without teams, the buzzers that registered. */
function buzzersCheck(t: Translate, view: GameView): CheckItem {
  const players = view.players.length;
  const unpaired = view.teams.filter((team) => team.buzzer === null).map((team) => team.name);
  if (unpaired.length > 0) {
    const more = unpaired.length - LISTED_TEAMS;
    const names = unpaired.slice(0, LISTED_TEAMS).join(", ") + (more > 0 ? ` ${t("launchMore", { n: more })}` : "");
    const text = t("launchTeamsUnpaired", { n: unpaired.length, total: view.teams.length, names, players });
    return { part: "buzzers", state: "warn", text, fixes: ["teamsTab"] };
  }
  if (view.teams.length > 0) return { part: "buzzers", state: "ok", text: t("launchTeamsPaired", { players }), fixes: [] };
  if (players === 0) return { part: "buzzers", state: "warn", text: t("launchNoBuzzers"), fixes: [] };
  return { part: "buzzers", state: "ok", text: t("launchBuzzersRegistered", { players }), fixes: [] };
}

/** The hall should see the projector full screen on a screen of its own. */
function projectorCheck(t: Translate, place: ProjectorWindow | null | undefined): CheckItem {
  if (place === undefined) return checking(t, "projector");
  if (place === null) return { part: "projector", state: "problem", text: t("launchProjectorClosed"), fixes: ["openProjector"] };
  if (place.monitors < 2) return { part: "projector", state: "warn", text: t("launchProjectorOneScreen"), fixes: [] };
  if (place.onDirectorMonitor) return { part: "projector", state: "warn", text: t("launchProjectorOnDirector"), fixes: ["placeProjector"] };
  if (!place.fullscreen) return { part: "projector", state: "warn", text: t("launchProjectorWindowed"), fixes: ["placeProjector"] };
  return { part: "projector", state: "ok", text: t("launchProjectorReady"), fixes: [] };
}

/** Only an ear in the hall tells that the sound gets there: the quizmaster plays a test sound and says so. */
function soundCheck(t: Translate, muted: Muted, sound: SoundTry): CheckItem {
  if (muted === "player") return { part: "sound", state: "warn", text: t("launchSoundPlayerMuted"), fixes: ["mediaTab"] };
  if (muted === "quiz") return { part: "sound", state: "warn", text: t("launchSoundQuizMuted"), fixes: [] };
  switch (sound) {
    case "notPlayed":
      return { part: "sound", state: "todo", text: t("launchSoundTry"), fixes: ["playSound"] };
    case "played":
      return { part: "sound", state: "todo", text: t("launchSoundPlayed"), fixes: ["soundHeard", "playSound"] };
    case "heard":
      return { part: "sound", state: "ok", text: t("launchSoundHeard"), fixes: ["playSound"] };
  }
}

function diskCheck(t: Translate, bytes: number | null | undefined): CheckItem | null {
  if (bytes === undefined) return checking(t, "disk");
  if (bytes === null) return null;
  const size = diskSize(bytes);
  if (bytes < DISK_PROBLEM_BYTES) return { part: "disk", state: "problem", text: t("launchDiskFull", { size }), fixes: [] };
  if (bytes < DISK_WARN_BYTES) return { part: "disk", state: "warn", text: t("launchDiskLow", { size }), fixes: [] };
  return { part: "disk", state: "ok", text: t("launchDiskFree", { size }), fixes: [] };
}

/**
 * The monitor the fix "full screen on its screen" opens the projector on:
 * the chosen one, unless the Director is on it, as it is when it was moved
 * there or the system opened it there; else none, so the backend picks one
 * the Director is not on (`projector_place`).
 */
export function projectorScreen(monitors: readonly MonitorInfo[], chosen: string | null): string | null {
  return monitors.some((monitor) => monitor.id === chosen && !monitor.director) ? chosen : null;
}

const LAUNCH_CHECK_KEY = "quiz-show.launch-check";

export interface LaunchCheckSetting {
  /** The check shows before the first slide. */
  on: boolean;
  setOn: (on: boolean) => void;
}

/** Whether the Director shows the check, kept per computer (preferences.ts): on unless turned off. */
export function useLaunchCheckSetting(): LaunchCheckSetting {
  const [on, setOnState] = useState(() => readPreference(LAUNCH_CHECK_KEY) !== "off");
  const setOn = useCallback((next: boolean) => {
    setOnState(next);
    writePreference(LAUNCH_CHECK_KEY, next ? "on" : "off");
  }, []);
  return useMemo(() => ({ on, setOn }), [on, setOn]);
}
