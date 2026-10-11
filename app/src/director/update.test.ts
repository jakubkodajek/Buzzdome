import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import { installQuestion, installStatus } from "./update";

const t = translator("en");

describe("installQuestion", () => {
  it("adds the release notes when the release has some", () => {
    expect(installQuestion(t, { version: "0.2.0", notes: "" })).toBe(t("updateConfirm", { version: "0.2.0" }));
    expect(installQuestion(t, { version: "0.2.0", notes: "Fixes the podium." })).toBe(
      `${t("updateConfirm", { version: "0.2.0" })}\n\n${t("updateNotes")}\nFixes the podium.`,
    );
  });
});

describe("installStatus", () => {
  it("counts the download in whole percent, then says it installs", () => {
    expect(installStatus(t, "0.2.0", null)).toBe(t("updateDownloading", { version: "0.2.0" }));
    expect(installStatus(t, "0.2.0", { downloaded: 999, total: 1_000 })).toBe(t("updateDownloadingPercent", { version: "0.2.0", percent: 99 }));
    expect(installStatus(t, "0.2.0", { downloaded: 1_000, total: 1_000 })).toBe(t("updateInstalling", { version: "0.2.0" }));
  });

  it("gives no percent when the server does not say the size", () => {
    expect(installStatus(t, "0.2.0", { downloaded: 5_000, total: null })).toBe(t("updateDownloading", { version: "0.2.0" }));
    expect(installStatus(t, "0.2.0", { downloaded: 0, total: 0 })).toBe(t("updateDownloading", { version: "0.2.0" }));
  });
});
