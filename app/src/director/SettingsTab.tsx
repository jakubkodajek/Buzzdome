import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type { Lang, Translate } from "../i18n";
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

export function useProjectorSettings(onError: (m: string) => void): ProjectorSettings {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState(0);
  const [fullscreen, setFullscreen] = useState(true);
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    api
      .listMonitors()
      .then((list) => {
        setMonitors(list);
        // The second display is usually the projector.
        if (list.length > 1) setMonitor(list[1].index);
      })
      .catch((e) => onError(String(e)));
    api.hostWindowOpen().then(setIsOpen);
  }, [onError]);

  const toggle = useCallback(() => {
    const action = isOpen ? api.closeHostWindow() : api.openHostWindow(monitor, fullscreen);
    action.then(() => setIsOpen(!isOpen)).catch((e) => onError(String(e)));
  }, [isOpen, monitor, fullscreen, onError]);

  return useMemo(
    () => ({ monitors, monitor, setMonitor, fullscreen, setFullscreen, isOpen, toggle }),
    [monitors, monitor, fullscreen, isOpen, toggle],
  );
}

export function SettingsTab(props: { t: Translate; lang: Lang; setLang: (l: Lang) => void; projector: ProjectorSettings }) {
  const { t, lang, setLang, projector } = props;
  return (
    <div className="settings">
      <fieldset>
        <legend>{t("projector")}</legend>
        <label>
          <span>Monitor</span>
          <select value={projector.monitor} onChange={(e) => projector.setMonitor(Number(e.target.value))}>
            {projector.monitors.map((m) => (
              <option key={m.index} value={m.index}>
                {m.name} ({m.width}×{m.height})
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
