import { describe, expect, it } from "vitest";
import { NO_CONTROLS, previewView } from "./host/preview";
import { translator } from "./i18n";
import { commandForKey, keyHint, KEYS } from "./keyboard";
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
