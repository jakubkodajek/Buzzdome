import { useState } from "react";
import { api } from "../api";
import { useApp, useReport } from "../app-context";
import { confirmAction } from "../confirm";
import { useAvailableUpdate } from "../hooks";
import type { UpdateProgress } from "../types";
import { installQuestion, installStatus, waitsForShow } from "./update";

/**
 * Offers the newer version of the app the update server has, while no show
 * runs: installing it ends the app. Without internet it offers nothing (the
 * backend logs why) until a later check gets an answer.
 */
export function UpdateBanner({ showRunning }: { showRunning: boolean }) {
  const { t, lang, caution } = useApp();
  const report = useReport();
  const update = useAvailableUpdate(showRunning);
  // The download's progress while installing (null before its first report); undefined otherwise.
  const [progress, setProgress] = useState<UpdateProgress | null | undefined>(undefined);
  const installing = progress !== undefined;
  if (!update || (showRunning && !installing)) return null;

  const install = () => {
    confirmAction(t, installQuestion(t, lang, update), t("updateInstall"))
      .then((yes) => {
        if (!yes) return;
        setProgress(null);
        // On success the app ends and starts again in the new version.
        return api.updateInstall(setProgress);
      })
      .catch((error: unknown) => {
        if (waitsForShow(error)) caution(t("updateAfterShow", { version: update.version }));
        else report(error);
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
