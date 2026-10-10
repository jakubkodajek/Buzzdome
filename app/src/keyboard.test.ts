import { describe, expect, it } from "vitest";
import { NO_CONTROLS, previewView } from "./host/preview";
import { translator } from "./i18n";
import { commandForKey, isFieldTarget, isHeldButtonPress, isTypingTarget, keyHint, KEYS } from "./keyboard";
import type { Controls, GameView } from "./types";

const slide = { id: "q1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;

function viewWith(controls: Partial<Controls>, leaderboardVisible = false): GameView {
  return { ...previewView({ ...slide }, 0, 1, "Quiz"), controls: { ...NO_CONTROLS, ...controls }, leaderboardVisible };
}

describe("commandForKey", () => {
  it("presses the keyboard buzzers 1 to 10 in any step", () => {
    expect(commandForKey("1", viewWith({}))).toEqual({ type: "press", buzzer: 1, button: "big" });
    expect(commandForKey("0", viewWith({}))).toEqual({ type: "press", buzzer: 10, button: "big" });
  });

  it("sends a step command only when the step takes it", () => {
    for (const key of [" ", "ArrowRight", "PageDown"]) {
      expect(commandForKey(key, viewWith({ next: true }))).toEqual({ type: "next" });
      expect(commandForKey(key, viewWith({ judge: true }))).toBeNull();
    }
    expect(commandForKey("Y", viewWith({ judge: true }))).toEqual({ type: "judgeRight" });
    expect(commandForKey("n", viewWith({ judge: true }))).toEqual({ type: "judgeWrong" });
    expect(commandForKey("y", viewWith({ next: true }))).toBeNull();
    expect(commandForKey("p", viewWith({ pauseClock: true }))).toEqual({ type: "pauseClock" });
    expect(commandForKey("p", viewWith({ resumeClock: true }))).toEqual({ type: "resumeClock" });
    expect(commandForKey("p", viewWith({ next: true }))).toBeNull();
  });

  it("toggles the leaderboard", () => {
    expect(commandForKey("s", viewWith({}))).toEqual({ type: "showLeaderboard", visible: true });
    expect(commandForKey("s", viewWith({}, true))).toEqual({ type: "showLeaderboard", visible: false });
    expect(commandForKey("x", viewWith({ next: true }))).toBeNull();
  });
});

describe("keyHint", () => {
  it("writes keys as on the keyboard, in the quizmaster's language", () => {
    expect(keyHint(translator("cs"), KEYS.next)).toBe("Mezerník");
    expect(keyHint(translator("en"), KEYS.right)).toBe("Y");
  });
});

describe("isHeldButtonPress", () => {
  const press = (key: string, repeat: boolean, tagName: string) => ({ key, repeat, target: { tagName } as unknown as EventTarget });

  it("takes Enter held on a button, not its first press", () => {
    expect(isHeldButtonPress(press("Enter", true, "BUTTON"))).toBe(true);
    expect(isHeldButtonPress(press("Enter", false, "BUTTON"))).toBe(false);
  });

  it("leaves other keys and other targets alone", () => {
    expect(isHeldButtonPress(press("a", true, "BUTTON"))).toBe(false);
    expect(isHeldButtonPress(press("Enter", true, "TEXTAREA"))).toBe(false);
  });
});

describe("isTypingTarget", () => {
  /** A key press's target, as the parts of an element the checks read. */
  const target = (tagName: string, more: { type?: string; isContentEditable?: boolean } = {}) => ({ tagName, ...more }) as unknown as EventTarget;

  it("takes text fields, text areas and editable text as typing", () => {
    for (const type of ["text", "search", "number"]) expect(isTypingTarget(target("INPUT", { type }))).toBe(true);
    expect(isTypingTarget(target("TEXTAREA"))).toBe(true);
    expect(isTypingTarget(target("DIV", { isContentEditable: true }))).toBe(true);
  });

  it("leaves the show its keys on sliders, checkboxes, lists and buttons", () => {
    for (const type of ["range", "checkbox", "radio", "color"]) expect(isTypingTarget(target("INPUT", { type }))).toBe(false);
    expect(isTypingTarget(target("SELECT"))).toBe(false);
    expect(isTypingTarget(target("BUTTON"))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });

  it("still counts sliders and lists as fields that keep their keys in the editor", () => {
    expect(isFieldTarget(target("INPUT", { type: "range" }))).toBe(true);
    expect(isFieldTarget(target("SELECT"))).toBe(true);
    expect(isFieldTarget(target("TEXTAREA"))).toBe(true);
    expect(isFieldTarget(target("BUTTON"))).toBe(false);
  });
});
