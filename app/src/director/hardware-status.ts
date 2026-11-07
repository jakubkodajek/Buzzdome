// The receiver and the stage lights during a show (spec BZ-3): what does
// not work as set up, or what the quizmaster set on purpose and may forget,
// shows in the Director's status bar whichever tab is open, and a part that
// stops working mid-show says so once.

import { useEffect, useRef, useState } from "react";
import { useApp } from "../app-context";
import { useLatest, useLightsStatus, useReceiverInfo } from "../hooks";
import type { StringKey, Translate } from "../i18n";
import { awaitsAnswer, outputErrorText, receiverStatusText } from "../messages";
import type { LightsStatus, ReceiverInfo } from "../types";

/** A part of the hardware the status bar names, with the tab that shows and puts it right. */
export interface HardwareChip {
  tab: "receiver" | "lights";
  /** warn: it does not work as set up; info: it works as the quizmaster set it, which the hall sees. */
  tone: "warn" | "info";
  label: StringKey;
  /** The part's state in full, for the tooltip; null where the label says it all. */
  detail: string | null;
}

/** Whether the receiver hears the buzzers now. */
export function receiverReady(info: ReceiverInfo | null): boolean {
  return info?.status?.kind === "connected" && !awaitsAnswer(info.status.protocol);
}

/**
 * The receiver's chip: when it fails, waits for its base station, or is
 * gone after it worked (`wasReady`). One that was never plugged in, is
 * closed or is not available on this computer gives none: a show plays
 * without hardware, and the check before the show said so.
 */
export function receiverChip(t: Translate, info: ReceiverInfo | null, wasReady: boolean): HardwareChip | null {
  const status = info?.status;
  if (!info || !status) return null;
  const chip = (label: StringKey): HardwareChip => ({ tab: "receiver", tone: "warn", label, detail: receiverStatusText(t, status, info.platform) });
  switch (status.kind) {
    case "connected":
      return awaitsAnswer(status.protocol) ? chip("receiverChipNotReady") : null;
    case "error":
      return chip("receiverChipFailed");
    case "looking":
    case "searching":
      return wasReady ? chip("receiverChipLost") : null;
    case "off":
      return null;
  }
}

/** Whether the lights' output takes what the show sends. */
export function lightsWork(status: LightsStatus | null): boolean {
  return status?.output.kind !== "lost";
}

/** The stage lights' chips: an output that does not take what is sent, a blackout, a held look. */
export function lightsChips(t: Translate, status: LightsStatus | null): HardwareChip[] {
  if (!status) return [];
  const chips: HardwareChip[] = [];
  const { output } = status;
  if (output.kind === "lost") chips.push({ tab: "lights", tone: "warn", label: "lightsChipLost", detail: t("lightsLost", { problem: outputErrorText(t, output.error) }) });
  if (status.blackout) chips.push({ tab: "lights", tone: "info", label: "lightsChipBlackout", detail: null });
  else if (status.hold !== null) chips.push({ tab: "lights", tone: "info", label: "lightsChipHeld", detail: null });
  return chips;
}

/**
 * The hardware's chips for the status bar, and one caution when the
 * receiver or the lights stop working while a show runs (`showRuns`).
 */
export function useHardwareStatus(showRuns: boolean): HardwareChip[] {
  const { t, caution } = useApp();
  const info = useReceiverInfo();
  const lights = useLightsStatus();
  const ready = receiverReady(info);
  // Whether the receiver has worked since the Director opened: one gone since is unplugged.
  const [everReady, setEverReady] = useState(ready);
  if (ready && !everReady) setEverReady(true);

  const latest = useLatest({ t, caution, showRuns, info, lights });
  const receiverWas = useRef(ready);
  useEffect(() => {
    const { t, caution, showRuns, info } = latest.current;
    if (receiverWas.current && !ready && showRuns && info?.status) caution({ text: t("receiverStopped"), detail: receiverStatusText(t, info.status, info.platform) });
    receiverWas.current = ready;
  }, [ready, latest]);
  const working = lightsWork(lights);
  const lightsWere = useRef(working);
  useEffect(() => {
    const { t, caution, showRuns, lights } = latest.current;
    if (lightsWere.current && !working && showRuns && lights?.output.kind === "lost") caution({ text: t("lightsStopped"), detail: outputErrorText(t, lights.output.error) });
    lightsWere.current = working;
  }, [working, latest]);

  const receiver = receiverChip(t, info, everReady);
  return [...(receiver ? [receiver] : []), ...lightsChips(t, lights)];
}
