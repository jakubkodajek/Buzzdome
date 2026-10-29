import { useEffect, useState } from "react";
import { api } from "../api";
import { useApp, useReport } from "../app-context";
import { confirmAction } from "../confirm";
import type { AvailableUpdate, UpdateProgress } from "../types";
import { installQuestion, installStatus, waitsForShow } from "./update";

/**
 * Offers the newer version of the app the update server has, while no show
 * runs: installing it ends the app. Without internet it offers nothing (the
 * backend logs why).
 */
export function UpdateBanner({ showRunning }: { showRunning: boolean }) {
  const { t } = useApp();
  const report = useReport();
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  // The download's progress while installing (null before its first report); undefined otherwise.
  const [progress, setProgress] = useState<UpdateProgress | null | undefined>(undefined);
  useEffect(() => {
    let alive = true;
    api
      .updateCheck()
      .then((found) => {
        if (alive) setUpdate(found);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  const installing = progress !== undefined;
  if (!update || (showRunning && !installing)) return null;

  const install = () => {
    confirmAction(t, installQuestion(t, update))
      .then((yes) => {
        if (!yes) return;
        setProgress(null);
        // On success the app ends and starts again in the new version.
        return api.updateInstall(setProgress);
      })
      .catch((error: unknown) => {
        if (!waitsForShow(error)) report(error);
      })
      .finally(() => setProgress(undefined));
  };

  return (
    <div className="update-banner" role="status">
      {installing ? (
        <span>{installStatus(t, update.version, progress)}</span>
      ) : (
        <>
          <span>{t("updateAvailable", { version: update.version })}</span>
          <button onClick={install}>{t("updateInstall")}</button>
        </>
      )}
    </div>
  );
}
