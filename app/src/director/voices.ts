// What the voices download or removal in the Settings is doing (`VoicesSettings`).

import { diskSize } from "../format";
import type { Translate } from "../i18n";
import type { VoiceDownload, VoiceModel } from "../types";

/**
 * A download or removal under way: connecting until the first bytes come,
 * downloading with how far it got, unpacking once all came, cancelling
 * once the quizmaster asked to (until the download ends), or removing.
 */
export type VoicesWork = { phase: "connecting" } | { phase: "downloading"; got: VoiceDownload } | { phase: "unpacking" } | { phase: "cancelling" } | { phase: "removing" };

/** The work after the download reports `got`: a cancel asked for stays until the download ends. */
export function downloadReported(work: VoicesWork | null, got: VoiceDownload): VoicesWork {
  if (work?.phase === "cancelling") return work;
  return got.received < got.total ? { phase: "downloading", got } : { phase: "unpacking" };
}

/** Whether the download can still be cancelled: until its bytes are all in. */
export function cancellable(work: VoicesWork | null): boolean {
  return work?.phase === "connecting" || work?.phase === "downloading" || work?.phase === "cancelling";
}

/** The status line: what `work` does, or, with nothing under way, whether `model` is downloaded. */
export function voicesStatus(t: Translate, work: VoicesWork | null, model: VoiceModel | null): string {
  switch (work?.phase) {
    case "connecting":
      return t("voicesConnecting");
    case "downloading":
      return t("voicesDownloading", {
        percent: Math.floor((100 * work.got.received) / work.got.total),
        received: diskSize(work.got.received),
        total: diskSize(work.got.total),
      });
    case "unpacking":
      return t("voicesUnpacking");
    case "cancelling":
      return t("voicesCancelling");
    case "removing":
      return t("voicesRemoving");
    case undefined:
      return t(model?.installed ? "voicesInstalled" : "voicesNotInstalled");
  }
}
