import { useState, type ReactNode } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { defaultChoice } from "../audio/library";
import { useSoundPreview } from "../audio/show-sounds";
import { DEFAULT_QUIZ_SOUNDS } from "../bindings/Constants";
import { useBackendEvent, usePlayerVolume, useProjectorWindow, useReceiverInfo, useShowDiskSpace } from "../hooks";
import type { StringKey } from "../i18n";
import { CheckIcon, CrossIcon, QuestionIcon, WarningIcon } from "../icons";
import { soundsOf } from "../quiz";
import type { GameView, Quiz } from "../types";
import { launchCheck, projectorScreen, type CheckFix, type CheckPart, type CheckState, type Muted, type SoundTry } from "./launch-check";
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
 * before its first slide. Hide puts it away until another show starts.
 */
export function LaunchCheck(props: LaunchCheckProps) {
  const [hidden, setHidden] = useState(false);
  useBackendEvent("show-changed", () => setHidden(false));
  // Hidden, it reads nothing: the panel polls while it is on screen.
  return hidden ? null : <LaunchCheckPanel {...props} onHide={() => setHidden(true)} />;
}

const PART_LABEL: Record<CheckPart, StringKey> = {
  receiver: "launchPartReceiver",
  buzzers: "launchPartBuzzers",
  projector: "launchPartProjector",
  sound: "launchPartSound",
  disk: "launchPartDisk",
};

const STATE_ICON: Record<CheckState, ReactNode> = {
  ok: <CheckIcon />,
  warn: <WarningIcon />,
  problem: <CrossIcon />,
  todo: <QuestionIcon />,
};

const FIX_LABEL: Record<CheckFix, StringKey> = {
  receiverTab: "tabReceiver",
  teamsTab: "tabTeams",
  mediaTab: "tabMedia",
  openProjector: "launchOpenProjector",
  placeProjector: "launchPlaceProjector",
  playSound: "launchPlaySound",
  soundHeard: "launchSoundHeardButton",
};

function LaunchCheckPanel({ view, quiz, projector, onTab, onHide }: LaunchCheckProps & { onHide: () => void }) {
  const { t } = useApp();
  const run = useRun();
  const receiver = useReceiverInfo();
  const projectorWindow = useProjectorWindow();
  const freeDiskBytes = useShowDiskSpace();
  const sounds = quiz ? soundsOf(quiz) : DEFAULT_QUIZ_SOUNDS;
  // The test sound plays as loud as the show's effects.
  const preview = useSoundPreview(sounds);
  const playerVolume = usePlayerVolume();
  const muted: Muted = playerVolume === 0 ? "player" : sounds.effectsVolume === 0 ? "quiz" : null;
  const [sound, setSound] = useState<SoundTry>("notPlayed");

  const items = launchCheck(t, { receiver, view, projector: projectorWindow, muted, sound, freeDiskBytes });
  const open = items.filter((item) => item.state !== "ok").length;

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
        // The app's own sound, so a quiz whose sound for the moment is missing still tests the speakers.
        preview.play("correct", defaultChoice("correct"));
        return setSound((tried) => (tried === "heard" ? tried : "played"));
      case "soundHeard":
        return setSound("heard");
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
                  {t(FIX_LABEL[action])}
                </button>
              ))}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
