import { describe, expect, it } from "vitest";
import { toastMs, trimToasts } from "./Toasts";

describe("toastMs", () => {
  it("keeps news for a moment and a failure longer, the longer its text", () => {
    expect(toastMs({ tone: "info", text: "Saved" }, false)).toBe(4000);
    expect(toastMs({ tone: "error", text: "No" }, false)).toBe(7000);
    expect(toastMs({ tone: "warn", text: "x".repeat(1000) }, false)).toBe(15000);
  });

  it("keeps a failure during a show until the quizmaster closes it", () => {
    expect(toastMs({ tone: "error", text: "No" }, true)).toBeNull();
    expect(toastMs({ tone: "warn", text: "Partly" }, true)).toBe(7000);
    expect(toastMs({ tone: "info", text: "Saved" }, true)).toBe(4000);
  });
});

describe("trimToasts", () => {
  const toast = (tone: "info" | "warn" | "error", text: string) => ({ tone, text });

  it("keeps up to three, dropping the oldest news first", () => {
    const shown = [toast("error", "Sound failed"), toast("info", "Saved"), toast("warn", "Partly"), toast("info", "Copied")];
    expect(trimToasts(shown).map((t) => t.text)).toEqual(["Sound failed", "Partly", "Copied"]);
    expect(trimToasts(shown.slice(0, 3))).toHaveLength(3);
  });

  it("drops the oldest failure only when only failures are left before the newest", () => {
    const shown = [toast("error", "A"), toast("error", "B"), toast("error", "C"), toast("error", "D")];
    expect(trimToasts(shown).map((t) => t.text)).toEqual(["B", "C", "D"]);
    const newest = [toast("error", "A"), toast("error", "B"), toast("error", "C"), toast("info", "Saved")];
    expect(trimToasts(newest).map((t) => t.text)).toEqual(["B", "C", "Saved"]);
    const caution = [toast("error", "A"), toast("error", "B"), toast("error", "C"), toast("warn", "Receiver stopped")];
    expect(trimToasts(caution).map((t) => t.text)).toEqual(["B", "C", "Receiver stopped"]);
  });

  it("never drops the newest message", () => {
    const shown = [toast("error", "A"), toast("info", "Saved"), toast("error", "B"), toast("info", "Copied"), toast("warn", "Partly")];
    expect(trimToasts(shown).map((t) => t.text)).toEqual(["A", "B", "Partly"]);
    expect(trimToasts(shown, 1).map((t) => t.text)).toEqual(["Partly"]);
  });
});
