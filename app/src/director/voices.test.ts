import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import type { VoiceModel } from "../types";
import { cancellable, downloadReported, voicesStatus, type VoicesWork } from "./voices";

const t = translator("en");
const model = (installed: boolean): VoiceModel => ({ installed, bytes: 122_000_000, voices: 10 });

describe("voices download", () => {
  it("connects until bytes come, then downloads, then unpacks", () => {
    const connecting: VoicesWork = { phase: "connecting" };
    expect(voicesStatus(t, connecting, model(false))).toBe("Connecting…");
    const downloading = downloadReported(connecting, { received: 50, total: 200 });
    expect(downloading).toEqual({ phase: "downloading", got: { received: 50, total: 200 } });
    expect(voicesStatus(t, downloading, model(false))).toMatch(/^Downloading the voices: 25%/);
    expect(downloadReported(downloading, { received: 200, total: 200 })).toEqual({ phase: "unpacking" });
  });

  it("keeps a cancel asked for while bytes still come", () => {
    const cancelling: VoicesWork = { phase: "cancelling" };
    expect(downloadReported(cancelling, { received: 60, total: 200 })).toBe(cancelling);
    expect(voicesStatus(t, cancelling, model(false))).toBe("Cancelling the download…");
  });

  it("offers Cancel from the start of the download until all its bytes are in", () => {
    expect(cancellable({ phase: "connecting" })).toBe(true);
    expect(cancellable({ phase: "downloading", got: { received: 1, total: 2 } })).toBe(true);
    expect(cancellable({ phase: "cancelling" })).toBe(true);
    expect(cancellable({ phase: "unpacking" })).toBe(false);
    expect(cancellable({ phase: "removing" })).toBe(false);
    expect(cancellable(null)).toBe(false);
  });

  it("says a removal removes, and with nothing under way whether the voices are in", () => {
    expect(voicesStatus(t, { phase: "removing" }, model(true))).toBe("Removing the voices…");
    expect(voicesStatus(t, null, model(true))).toBe(t("voicesInstalled"));
    expect(voicesStatus(t, null, model(false))).toBe(t("voicesNotInstalled"));
    expect(voicesStatus(t, null, null)).toBe(t("voicesNotInstalled"));
  });
});
