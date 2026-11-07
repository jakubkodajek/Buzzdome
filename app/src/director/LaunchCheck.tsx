import { useState, type ReactNode } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { defaultChoice } from "../audio/library";
import { useSoundPreview } from "../audio/show-sounds";
import { DEFAULT_QUIZ_SETTINGS, DEFAULT_QUIZ_SOUNDS } from "../bindings/Constants";
import { useBackendEvent, usePlayerVolume, useProjectorWindow, useReceiverInfo, useShowDiskSpace } from "../hooks";
import type { StringKey, Translate } from "../i18n";
import { CheckIcon, CrossIcon, QuestionIcon, WarningIcon } from "../icons";
import { LANGUAGE_LABEL, settingsOf, soundsOf } from "../quiz";
import { usePreparing, useSystemVoice } from "../audio/read-aloud";
import type { GameView, Quiz } from "../types";
import { launchCheck, projectorScreen, type CheckFix, type CheckItem, type CheckPart, type CheckState, type Muted, type SoundTry } from "./launch-check";
import type { ProjectorSettings } from "./SettingsTab";

/** The Director's tabs a fix opens. */
export type FixTab = "receiver" | "teams" | "media";

interface LaunchCheckProps {
  view: GameView;
  quiz: Quiz | null;
  projector: ProjectorSettings;
  onTab: (tab: FixTab) => void;
}

/**
 * The check before a show (spec SF-2), below the tabs while the show waits
 * before its first slide. Hide folds it into a button that shows it again,
 * until another show starts, and counts what is still to sort out.
 */
export function LaunchCheck(props: LaunchCheckProps) {
  const { t } = useApp();
  const [hidden, setHidden] = useState(false);
  useBackendEvent("show-changed", () => setHidden(false));
  // Read while hidden too, for the count on its button.
  const check = useLaunchItems(props.view, props.quiz);
  const open = check.items.filter((item) => item.state !== "ok").length;
  if (hidden) {
    return (
      <div className="launch-check-hidden">
        <button onClick={() => setHidden(false)}>{open === 0 ? t("launchShow") : t("launchShowOpen", { n: open })}</button>
      </div>
    );
  }
  return <LaunchCheckPanel {...props} check={check} open={open} onHide={() => setHidden(true)} />;
}

const PART_LABEL: Record<CheckPart, StringKey> = {
  receiver: "launchPartReceiver",
  buzzers: "launchPartBuzzers",
  projector: "launchPartProjector",
  sound: "launchPartSound",
  voice: "launchPartVoice",
  disk: "launchPartDisk",
};

const STATE_ICON: Record<CheckState, ReactNode> = {
  ok: <CheckIcon />,
  warn: <WarningIcon />,
  problem: <CrossIcon />,
  todo: <QuestionIcon />,
};

/** A fix's button: the tab it opens, or what it does. */
const FIX_LABEL: Record<CheckFix, StringKey | { tab: StringKey }> = {
  receiverTab: { tab: "tabReceiver" },
  teamsTab: { tab: "tabTeams" },
  mediaTab: { tab: "tabMedia" },
  openProjector: "openProjectorWindow",
  placeProjector: "launchPlaceProjector",
  playSound: "launchPlaySound",
  soundHeard: "launchSoundHeardButton",
};

function fixLabel(t: Translate, action: CheckFix): string {
  const label = FIX_LABEL[action];
  return typeof label === "string" ? t(label) : t("launchGoTo", { tab: t(label.tab) });
}

/** The check's parts as they stand, and how far the quizmaster got with the test sound. */
interface LaunchItems {
  items: CheckItem[];
  setSound: (update: (tried: SoundTry) => SoundTry) => void;
  /** Plays the app's test sound as loud as the show's effects. */
  playTestSound: () => void;
}

function useLaunchItems(view: GameView, quiz: Quiz | null): LaunchItems {
  const { t } = useApp();
  const receiver = useReceiverInfo();
  const projectorWindow = useProjectorWindow();
  const freeDiskBytes = useShowDiskSpace();
  const sounds = quiz ? soundsOf(quiz) : DEFAULT_QUIZ_SOUNDS;
  // The test sound plays as loud as the show's effects.
  const preview = useSoundPreview(sounds);
  const playerVolume = usePlayerVolume();
  const muted: Muted = playerVolume === 0 ? "player" : sounds.effectsVolume === 0 ? "quiz" : null;
  const [sound, setSound] = useState<SoundTry>("notPlayed");

  const preparing = usePreparing();
  const language = quiz ? settingsOf(quiz).language : DEFAULT_QUIZ_SETTINGS.language;
  const systemVoice = useSystemVoice(language);
  const voice = { preparing, systemVoice: systemVoice && systemVoice.name, language: t(LANGUAGE_LABEL[language]) };
  const items = launchCheck(t, { receiver, view, projector: projectorWindow, muted, sound, freeDiskBytes, voice });
  // The app's own sound, so a quiz whose sound for the moment is missing still tests the speakers.
  return { items, setSound, playTestSound: () => preview.play("correct", defaultChoice("correct")) };
}

function LaunchCheckPanel(props: LaunchCheckProps & { check: LaunchItems; open: number; onHide: () => void }) {
  const { projector, onTab, check, open, onHide } = props;
  const { items, setSound } = check;
  const { t } = useApp();
  const run = useRun();

  const fix = (action: CheckFix) => {
    switch (action) {
      case "receiverTab":
        return onTab("receiver");
      case "teamsTab":
        return onTab("teams");
      case "mediaTab":
        return onTab("media");
      case "openProjector":
        return run(api.openHostWindow(projector.monitor, projector.fullscreen));
      case "placeProjector":
        // Full screen on a screen the Director is not on: the monitors as they are now, as the Director may have moved.
        projector.setFullscreen(true);
        return run(api.listMonitors().then((monitors) => api.openHostWindow(projectorScreen(monitors, projector.monitor), true)));
      case "playSound":
        check.playTestSound();
        return setSound((tried) => (tried === "heard" ? tried : "played"));
      case "soundHeard":
        return setSound(() => "heard");
    }
  };

  return (
    <section className="launch-check" aria-label={t("launchCheck")}>
      <header>
        <h2>{t("launchCheck")}</h2>
        <span className="muted">{open === 0 ? t("launchAllReady") : t("launchOpen", { n: open })}</span>
        <button onClick={onHide}>{t("launchHide")}</button>
      </header>
      <ul>
        {items.map((item) => (
          <li key={item.part} className={`launch-item ${item.state}`}>
            <span className="launch-icon">{STATE_ICON[item.state]}</span>
            <strong>{t(PART_LABEL[item.part])}</strong>
            <span className="launch-text">{item.text}</span>
            <span className="launch-fixes">
              {item.fixes.map((action) => (
                <button key={action} onClick={() => fix(action)}>
                  {fixLabel(t, action)}
                </button>
              ))}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
