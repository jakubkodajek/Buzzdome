import { describe, expect, it } from "vitest";
import { toastMs } from "./Toasts";

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
