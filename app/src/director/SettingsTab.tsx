import { useCallback, useEffect, useMemo, useState, type KeyboardEvent } from "react";
import blueIcon from "../../src-tauri/icons/colors/buzzdome-blue.png";
import greenIcon from "../../src-tauri/icons/colors/buzzdome-green.png";
import orangeIcon from "../../src-tauri/icons/colors/buzzdome-orange.png";
import purpleIcon from "../../src-tauri/icons/colors/buzzdome-purple.png";
import redIcon from "../../src-tauri/icons/colors/buzzdome-red.png";
import yellowIcon from "../../src-tauri/icons/colors/buzzdome-yellow.png";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { useAppIcon, useAppVersion, useHostWindowOpen } from "../hooks";
import type { Lang, StringKey } from "../i18n";
import { readPreference, writePreference } from "../preferences";
import { keyboardHelp } from "../keyboard";
import { stepSelection } from "../media-player";
import type { IconColor, MonitorInfo } from "../types";
import type { LaunchCheckSetting } from "./launch-check";
import { VoicesSettings } from "./VoicesSettings";
import { LOOKS, readLook, type DirectorLook, type Look } from "./look";

/** The app's icons, as the windows take them (`app_icon.rs`), in the order the picker shows them. */
const ICONS: Record<IconColor, { label: StringKey; image: string }> = {
  orange: { label: "iconOrange", image: orangeIcon },
  blue: { label: "iconBlue", image: blueIcon },
  green: { label: "iconGreen", image: greenIcon },
  purple: { label: "iconPurple", image: purpleIcon },
  red: { label: "iconRed", image: redIcon },
  yellow: { label: "iconYellow", image: yellowIcon },
};

const ICON_COLORS = Object.keys(ICONS) as IconColor[];

/** The arrow keys of a row of choices as `stepSelection` reads a list's. */
const ROW_KEYS: Record<string, string> = { ArrowLeft: "ArrowUp", ArrowRight: "ArrowDown" };

const LOOK_LABEL: Record<Look, StringKey> = { light: "lookLight", dark: "lookDark", system: "lookSystem" };

export interface ProjectorSettings {
  monitors: MonitorInfo[];
  /** Lists the monitors again, such as one plugged in since. */
  refreshMonitors: () => void;
  /** The chosen monitor's `MonitorInfo.id`, or null for one the Director is not on. */
  monitor: string | null;
  fullscreen: boolean;
  /** Sets full screen, for the projector the caller opens next. */
  setFullscreen: (on: boolean) => void;
  /** Sets the monitor or full screen, and moves an open projector there at once. */
  choose: (change: { monitor?: string | null; fullscreen?: boolean }) => void;
  isOpen: boolean;
  toggle: () => void;
}

/** The projector's monitor (none for one the Director is not on) and full screen, kept on this computer, so they last a restart. */
const MONITOR_KEY = "quiz-show.projector-monitor";
const FULLSCREEN_KEY = "quiz-show.projector-fullscreen";

const storedMonitor = () => readPreference(MONITOR_KEY) || null;

export function useProjectorSettings(): ProjectorSettings {
  const run = useRun();
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitorState] = useState<string | null>(storedMonitor);
  const [fullscreen, setFullscreenState] = useState(() => readPreference(FULLSCREEN_KEY) !== "off");
  const isOpen = useHostWindowOpen();

  const refreshMonitors = useCallback(() => {
    run(
      api.listMonitors().then((list) => {
        setMonitors(list);
        // A chosen monitor that is gone gives way to one the Director is not on, and is chosen again once it is back.
        const kept = storedMonitor();
        setMonitorState(list.some((m) => m.id === kept) ? kept : null);
      }),
    );
  }, [run]);
  useEffect(() => refreshMonitors(), [refreshMonitors]);

  const setFullscreen = useCallback((on: boolean) => {
    setFullscreenState(on);
    writePreference(FULLSCREEN_KEY, on ? "on" : "off");
  }, []);

  // The backend moves an open projector as it opens a closed one.
  const choose = useCallback(
    (change: { monitor?: string | null; fullscreen?: boolean }) => {
      const next = { monitor: change.monitor === undefined ? monitor : change.monitor, fullscreen: change.fullscreen ?? fullscreen };
      setMonitorState(next.monitor);
      writePreference(MONITOR_KEY, next.monitor ?? "");
      setFullscreen(next.fullscreen);
      if (isOpen) run(api.openHostWindow(next.monitor, next.fullscreen));
    },
    [monitor, fullscreen, isOpen, run, setFullscreen],
  );

  // The backend lists the monitors again when it opens the projector.
  const toggle = useCallback(() => {
    run(isOpen ? api.closeHostWindow() : api.openHostWindow(monitor, fullscreen));
  }, [isOpen, monitor, fullscreen, run]);

  return useMemo(
    () => ({ monitors, refreshMonitors, monitor, fullscreen, setFullscreen, choose, isOpen, toggle }),
    [monitors, refreshMonitors, monitor, fullscreen, setFullscreen, choose, isOpen, toggle],
  );
}

export function SettingsTab({ projector, look, launch, showRunning }: { projector: ProjectorSettings; look: DirectorLook; launch: LaunchCheckSetting; showRunning: boolean }) {
  const { t, lang, setLang } = useApp();
  return (
    <div className="settings">
      <fieldset>
        <legend>{t("projector")}</legend>
        <label>
          <span>{t("monitor")}</span>
          <select
            value={projector.monitor ?? ""}
            onFocus={projector.refreshMonitors}
            onChange={(e) => projector.choose({ monitor: e.target.value || null })}
          >
            <option value="">{t("monitorAuto")}</option>
            {projector.monitors.map((m, i) => (
              <option key={m.id} value={m.id}>
                {m.name ?? t("displayN", { n: i + 1 })} ({m.width}×{m.height}){m.director && ` · ${t("monitorDirector")}`}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("fullscreen")}</span>
          <input type="checkbox" checked={projector.fullscreen} onChange={(e) => projector.choose({ fullscreen: e.target.checked })} />
        </label>
        {noSecondMonitor(projector) && <p className="muted">{t("monitorNoSecond")}</p>}
        <button onClick={projector.toggle}>{projector.isOpen ? t("closeProjectorWindow") : t("openProjectorWindow")}</button>
      </fieldset>
      <AppIconPicker />
      <fieldset>
        <legend>{t("language")}</legend>
        <select value={lang} onChange={(e) => setLang(e.target.value as Lang)}>
          <option value="cs">Čeština</option>
          <option value="en">English</option>
        </select>
      </fieldset>
      <fieldset>
        <legend>{t("directorLook")}</legend>
        <select value={look.look} onChange={(e) => look.setLook(readLook(e.target.value))}>
          {LOOKS.map((value) => (
            <option key={value} value={value}>
              {t(LOOK_LABEL[value])}
            </option>
          ))}
        </select>
        <p className="muted">{t("directorLookHint")}</p>
      </fieldset>
      <VoicesSettings showRunning={showRunning} />
      <fieldset>
        <legend>{t("launchCheck")}</legend>
        <label>
          <span>{t("launchCheckShow")}</span>
          <input type="checkbox" checked={launch.on} onChange={(e) => launch.setOn(e.target.checked)} />
        </label>
        <p className="muted">{t("launchCheckHint")}</p>
      </fieldset>
      <p className="muted">{keyboardHelp(t)}</p>
      <AppVersion />
    </div>
  );
}

/** Whether the projector would find no monitor but the Director's, so it opens in a window over the Director (`projector_place`). */
function noSecondMonitor({ monitors, monitor }: ProjectorSettings): boolean {
  return monitor === null && monitors.length > 0 && monitors.every((m) => m.director);
}

/** The app's version, which an update changes. */
function AppVersion() {
  const { t } = useApp();
  const version = useAppVersion();
  return version && <p className="muted">{t("appVersion", { name: t("appTitle"), version })}</p>;
}

/**
 * The colour of the windows' icon, orange unless another is chosen. A radio
 * group: one tab stop, the arrow keys, Home and End choose another.
 */
function AppIconPicker() {
  const { t } = useApp();
  const run = useRun();
  const current = useAppIcon();
  const focused = current ?? ICON_COLORS[0];
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const next = stepSelection(ICON_COLORS, focused, ROW_KEYS[e.key] ?? e.key) as IconColor | null;
    if (next === null) return;
    e.preventDefault();
    e.currentTarget.querySelector<HTMLButtonElement>(`[data-color="${next}"]`)?.focus();
    if (next !== current) run(api.setAppIcon(next));
  };
  return (
    <fieldset>
      <legend>{t("appIcon")}</legend>
      <div className="icon-picker" role="radiogroup" aria-label={t("appIcon")} onKeyDown={onKeyDown}>
        {ICON_COLORS.map((color) => (
          <button
            key={color}
            role="radio"
            data-color={color}
            tabIndex={color === focused ? 0 : -1}
            aria-checked={color === current}
            className={color === current ? "icon-choice chosen" : "icon-choice"}
            title={t(ICONS[color].label)}
            onClick={() => run(api.setAppIcon(color))}
          >
            <img src={ICONS[color].image} alt={t(ICONS[color].label)} />
          </button>
        ))}
      </div>
      <p className="muted">{t("appIconHint")}</p>
    </fieldset>
  );
}
