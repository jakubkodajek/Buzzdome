import { describe, expect, it, vi } from "vitest";
import { confirmAction, confirmEdits, openDialogs, saveChoice } from "./confirm";
import { translator } from "./i18n";

const answers: { resolve: (yes: boolean) => void; reject: (error: unknown) => void }[] = [];
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: () => new Promise<boolean>((resolve, reject) => answers.push({ resolve, reject })),
  message: () => Promise.resolve(pressed.shift() ?? ""),
}));
const pressed: string[] = [];
const t = translator("en");

describe("openDialogs", () => {
  it("counts each dialog while it is open, also one that fails", async () => {
    const first = confirmAction(t, "End the show?", "End show");
    const second = confirmAction(t, "Restart?");
    expect(openDialogs.get()).toBe(2);
    answers[0].resolve(true);
    await expect(first).resolves.toBe(true);
    expect(openDialogs.get()).toBe(1);
    answers[1].reject(new Error("no window"));
    await expect(second).rejects.toThrow("no window");
    expect(openDialogs.get()).toBe(0);
  });
});

describe("saveChoice", () => {
  const labels = { yes: "Uložit", no: "Neukládat" };

  it("reads the button pressed from its label", () => {
    expect(saveChoice("Uložit", labels)).toBe("save");
    expect(saveChoice("Neukládat", labels)).toBe("discard");
    expect(saveChoice("Zrušit", labels)).toBe("cancel");
  });

  it("cancels on any other answer, so nothing is lost unasked", () => {
    for (const answer of ["Cancel", "Yes", "No", "Ok", ""]) expect(saveChoice(answer, labels)).toBe("cancel");
  });
});

describe("confirmEdits", () => {
  it("reads the choice from the button pressed, and any other answer cancels", async () => {
    pressed.push(t("resumeWithEdits"), t("resumeAsBegun"), t("cancel"), "");
    const answers = [];
    for (let n = 0; n < 4; n++) answers.push(await confirmEdits(t, "Go on with the edits?"));
    expect(answers).toEqual(["edits", "asBegun", "cancel", "cancel"]);
  });
});
