import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { useHostWindowOpen } from "../hooks";
import type { Lang } from "../i18n";
import type { MonitorInfo } from "../types";

export interface ProjectorSettings {
  monitors: MonitorInfo[];
  monitor: number;
  setMonitor: (index: number) => void;
  fullscreen: boolean;
  setFullscreen: (on: boolean) => void;
  isOpen: boolean;
  toggle: () => void;
}

export function useProjectorSettings(): ProjectorSettings {
  const run = useRun();
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState(0);
  const [fullscreen, setFullscreen] = useState(true);
  const isOpen = useHostWindowOpen();

  useEffect(() => {
    run(
      api.listMonitors().then((list) => {
        setMonitors(list);
        // The second display is usually the projector.
        if (list.length > 1) setMonitor(list[1].index);
      }),
    );
  }, [run]);

  const toggle = useCallback(() => {
    run(isOpen ? api.closeHostWindow() : api.openHostWindow(monitor, fullscreen));
  }, [isOpen, monitor, fullscreen, run]);

  return useMemo(
    () => ({ monitors, monitor, setMonitor, fullscreen, setFullscreen, isOpen, toggle }),
    [monitors, monitor, fullscreen, isOpen, toggle],
  );
}

export function SettingsTab({ projector }: { projector: ProjectorSettings }) {
  const { t, lang, setLang } = useApp();
  return (
    <div className="settings">
      <fieldset>
        <legend>{t("projector")}</legend>
        <label>
          <span>{t("monitor")}</span>
          <select value={projector.monitor} onChange={(e) => projector.setMonitor(Number(e.target.value))}>
            {projector.monitors.map((m) => (
              <option key={m.index} value={m.index}>
                {m.name ?? t("displayN", { n: m.index + 1 })} ({m.width}×{m.height})
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
      <fieldset>
        <legend>{t("language")}</legend>
        <select value={lang} onChange={(e) => setLang(e.target.value as Lang)}>
          <option value="cs">Čeština</option>
          <option value="en">English</option>
        </select>
      </fieldset>
      <p className="muted">{t("keyboardHint")}</p>
    </div>
  );
}
