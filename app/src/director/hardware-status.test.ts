import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { DeviceInfo, LightsStatus, ReceiverInfo, ReceiverStatus } from "../types";
import { lightsChips, lightsWork, receiverChip, receiverReady } from "./hardware-status";

const t = translator("en");
const device: DeviceInfo = { path: "1", vendorId: 0x1234, productId: 0x5678, manufacturer: null, product: "Base", serialNumber: null, usagePage: 1, usage: 4, interfaceNumber: 0 };
const info = (status: ReceiverStatus | null) => ({ status, platform: "windows" }) as ReceiverInfo;
const connected: ReceiverStatus = { kind: "connected", device, protocol: { type: "enjoy303s", answered: true }, hasLights: true, learnable: false };
const waiting: ReceiverStatus = { ...connected, protocol: { type: "enjoy303s", answered: false } };
const looking: ReceiverStatus = { kind: "looking" };

describe("receiverChip", () => {
  it("says nothing while the receiver hears the buzzers, or before it is loaded", () => {
    expect(receiverReady(info(connected))).toBe(true);
    expect(receiverChip(t, info(connected), true)).toBeNull();
    expect(receiverChip(t, null, true)).toBeNull();
  });

  it("names a receiver that fails or waits for its base station, with the Receiver tab", () => {
    expect(receiverReady(info(waiting))).toBe(false);
    expect(receiverChip(t, info(waiting), false)).toMatchObject({ tab: "receiver", tone: "warn", label: "receiverChipNotReady" });
    const failed: ReceiverStatus = { kind: "error", problem: "cannotOpen", detail: "access denied" };
    expect(receiverChip(t, info(failed), false)).toMatchObject({ tab: "receiver", tone: "warn", label: "receiverChipFailed" });
  });

  it("names one unplugged after it worked, not one never plugged in, closed or not available here", () => {
    expect(receiverChip(t, info(looking), true)).toMatchObject({ label: "receiverChipLost" });
    expect(receiverChip(t, info({ kind: "searching", selector: { vendorId: 1, productId: 2, usagePage: null, interfaceNumber: null } }), true)).toMatchObject({ label: "receiverChipLost" });
    expect(receiverChip(t, info(looking), false)).toBeNull();
    expect(receiverChip(t, info({ kind: "off" }), true)).toBeNull();
    expect(receiverChip(t, info(null), true)).toBeNull();
  });
});

describe("lightsChips", () => {
  const status = (more: Partial<LightsStatus>): LightsStatus => ({ output: { kind: "connected", name: "Art-Net" }, messages: null, look: null, hold: null, blackout: false, ...more });

  it("says nothing while the lights take what the show sends, as it sends it", () => {
    expect(lightsChips(t, status({}))).toEqual([]);
    expect(lightsChips(t, status({ output: { kind: "off" } }))).toEqual([]);
    expect(lightsChips(t, null)).toEqual([]);
  });

  it("names an output that does not take what is sent, and a blackout or held look the quizmaster may forget", () => {
    const lost = status({ output: { kind: "lost", error: { problem: "notFound" } } });
    expect(lightsWork(lost)).toBe(false);
    expect(lightsChips(t, lost)).toMatchObject([{ tab: "lights", tone: "warn", label: "lightsChipLost" }]);
    expect(lightsChips(t, status({ blackout: true, hold: 2 }))).toMatchObject([{ tone: "info", label: "lightsChipBlackout" }]);
    expect(lightsChips(t, status({ hold: 2 }))).toMatchObject([{ tone: "info", label: "lightsChipHeld" }]);
  });
});
