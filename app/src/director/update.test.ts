import { describe, expect, it } from "vitest";
import { translator } from "../i18n";
import { installQuestion, installStatus, waitsForShow } from "./update";

const t = translator("en");

describe("installQuestion", () => {
  const notes = { cs: "Opravuje stupně vítězů.", en: "Fixes the podium." };

  it("adds the release notes when the release has some", () => {
    expect(installQuestion(t, "en", { version: "0.2.0", notes: { cs: "", en: "" }, aiApps: false })).toBe(t("updateConfirm", { version: "0.2.0" }));
    expect(installQuestion(t, "en", { version: "0.2.0", notes, aiApps: false })).toBe(
      `${t("updateConfirm", { version: "0.2.0" })}\n\n${t("updateNotes")}\nFixes the podium.`,
    );
  });

  it("gives the notes in the quizmaster's language", () => {
    const cs = translator("cs");
    expect(installQuestion(cs, "cs", { version: "0.2.0", notes, aiApps: false })).toBe(
      `${cs("updateConfirm", { version: "0.2.0" })}\n\n${cs("updateNotes")}\nOpravuje stupně vítězů.`,
    );
  });

  it("says to open the AI apps again when one runs the app's AI tools", () => {
    expect(installQuestion(t, "en", { version: "0.2.0", notes, aiApps: true })).toBe(
      `${t("updateConfirm", { version: "0.2.0" })} ${t("updateRestartAiApps")}\n\n${t("updateNotes")}\nFixes the podium.`,
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

describe("waitsForShow", () => {
  it("lets only a show begun during the download put the update off", () => {
    expect(waitsForShow({ code: "showRunning" })).toBe(true);
    expect(waitsForShow({ code: "updateFailed", detail: "signature" })).toBe(false);
    expect(waitsForShow(new Error("showRunning"))).toBe(false);
  });
});
