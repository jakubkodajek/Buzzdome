import { getVersion } from "@tauri-apps/api/app";
import { useCallback, useEffect, useMemo, useState } from "react";
import blueIcon from "../../src-tauri/icons/colors/buzzdome-blue.png";
import greenIcon from "../../src-tauri/icons/colors/buzzdome-green.png";
import orangeIcon from "../../src-tauri/icons/colors/buzzdome-orange.png";
import purpleIcon from "../../src-tauri/icons/colors/buzzdome-purple.png";
import redIcon from "../../src-tauri/icons/colors/buzzdome-red.png";
import yellowIcon from "../../src-tauri/icons/colors/buzzdome-yellow.png";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { useAppIcon, useHostWindowOpen } from "../hooks";
import type { Lang, StringKey } from "../i18n";
import type { IconColor, MonitorInfo } from "../types";
import type { LaunchCheckSetting } from "./launch-check";
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

const LOOK_LABEL: Record<Look, StringKey> = { light: "lookLight", dark: "lookDark", system: "lookSystem" };

export interface ProjectorSettings {
  monitors: MonitorInfo[];
  /** Lists the monitors again, such as one plugged in since. */
  refreshMonitors: () => void;
  /** The chosen monitor's `MonitorInfo.id`, or null for one the Director is not on. */
  monitor: string | null;
  setMonitor: (id: string | null) => void;
  fullscreen: boolean;
  setFullscreen: (on: boolean) => void;
  isOpen: boolean;
  toggle: () => void;
}

export function useProjectorSettings(): ProjectorSettings {
  const run = useRun();
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState<string | null>(null);
  const [fullscreen, setFullscreen] = useState(true);
  const isOpen = useHostWindowOpen();

  const refreshMonitors = useCallback(() => {
    run(
      api.listMonitors().then((list) => {
        setMonitors(list);
        // A chosen monitor that is gone gives way to one the Director is not on.
        setMonitor((chosen) => (list.some((m) => m.id === chosen) ? chosen : null));
      }),
    );
  }, [run]);
  useEffect(() => refreshMonitors(), [refreshMonitors]);

  // The backend lists the monitors again when it opens the projector.
  const toggle = useCallback(() => {
    run(isOpen ? api.closeHostWindow() : api.openHostWindow(monitor, fullscreen));
  }, [isOpen, monitor, fullscreen, run]);

  return useMemo(
    () => ({ monitors, refreshMonitors, monitor, setMonitor, fullscreen, setFullscreen, isOpen, toggle }),
    [monitors, refreshMonitors, monitor, fullscreen, isOpen, toggle],
  );
}

export function SettingsTab({ projector, look, launch }: { projector: ProjectorSettings; look: DirectorLook; launch: LaunchCheckSetting }) {
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
            onChange={(e) => projector.setMonitor(e.target.value || null)}
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
          <input type="checkbox" checked={projector.fullscreen} onChange={(e) => projector.setFullscreen(e.target.checked)} />
        </label>
        <button onClick={projector.toggle}>{projector.isOpen ? t("closeProjector") : t("openProjector")}</button>
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
      <fieldset>
        <legend>{t("launchCheck")}</legend>
        <label>
          <span>{t("launchCheckShow")}</span>
          <input type="checkbox" checked={launch.on} onChange={(e) => launch.setOn(e.target.checked)} />
        </label>
        <p className="muted">{t("launchCheckHint")}</p>
      </fieldset>
      <p className="muted">{t("keyboardHint")}</p>
      <AppVersion />
    </div>
  );
}

/** The app's version, which an update changes. */
function AppVersion() {
  const { t } = useApp();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    getVersion()
      .then((v) => {
        if (alive) setVersion(v);
      })
      .catch((e) => console.error(e));
    return () => {
      alive = false;
    };
  }, []);
  return version && <p className="muted">{t("appVersion", { name: t("appTitle"), version })}</p>;
}

/** The colour of the windows' icon, orange unless another is chosen. */
function AppIconPicker() {
  const { t } = useApp();
  const run = useRun();
  const current = useAppIcon();
  return (
    <fieldset>
      <legend>{t("appIcon")}</legend>
      <div className="icon-picker" role="radiogroup" aria-label={t("appIcon")}>
        {(Object.keys(ICONS) as IconColor[]).map((color) => (
          <button
            key={color}
            role="radio"
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
