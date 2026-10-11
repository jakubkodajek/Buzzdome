// What the Director says of a newer version of the app (`UpdateBanner`).

import type { Translate } from "../i18n";
import { isAppError } from "../messages";
import type { AvailableUpdate, UpdateProgress } from "../types";

/** The question that confirms installing `update`, with what its release says changed. */
export function installQuestion(t: Translate, update: AvailableUpdate): string {
  const question = t("updateConfirm", { version: update.version });
  return update.notes ? `${question}\n\n${t("updateNotes")}\n${update.notes}` : question;
}

/**
 * How installing `version` goes, given how much of it has been downloaded:
 * null before the first progress report.
 */
export function installStatus(t: Translate, version: string, progress: UpdateProgress | null): string {
  const total = progress?.total;
  if (!progress || !total) return t("updateDownloading", { version });
  if (progress.downloaded >= total) return t("updateInstalling", { version });
  return t("updateDownloadingPercent", { version, percent: Math.floor((progress.downloaded * 100) / total) });
}

/**
 * Whether installing was refused because a show began during the download:
 * the update then waits until the show ends, when the banner offers it again.
 */
export function waitsForShow(error: unknown): boolean {
  return isAppError(error) && error.code === "showRunning";
}
