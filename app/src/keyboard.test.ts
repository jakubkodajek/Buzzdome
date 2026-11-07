import { describe, expect, it } from "vitest";
import { NO_CONTROLS, previewView } from "./host/preview";
import { translator } from "./i18n";
import { KEY_BUZZERS, PROJECTOR_COMMANDS } from "./bindings/ProjectorCommands";
import { commandForAction, commandForKey, commandForRemote, isFieldTarget, isHeldButtonPress, isPlainKey, isProjectorCloseKey, isTypingTarget, keyCombo, keyHint, KEYS, showKey } from "./keyboard";
import type { Controls, GameView, Phase } from "./types";

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

  it("takes the digits by their place, so the top row presses buzzers on a Czech layout too", () => {
    expect(showKey({ key: "ě", code: "Digit2" })).toBe("2");
    expect(showKey({ key: "é", code: "Digit0" })).toBe("0");
    expect(showKey({ key: "7", code: "Numpad7" })).toBe("7");
    expect(showKey({ key: "y", code: "KeyZ" })).toBe("y");
    expect(commandForKey(showKey({ key: "+", code: "Digit1" }), viewWith({}))).toEqual({ type: "press", buzzer: 1, button: "big" });
  });

  it("goes back a slide with Left and Page Up in the Director only, when the show may jump", () => {
    const view = { ...viewWith({ goTo: true }), previousSlide: 3 };
    for (const key of ["ArrowLeft", "PageUp"]) {
      expect(commandForKey(key, view, true)).toEqual({ type: "goTo", slide: 3 });
      expect(commandForKey(key, view)).toBeNull();
      expect(commandForKey(key, { ...view, controls: { ...view.controls, goTo: false } }, true)).toBeNull();
    }
  });

  it("gives only commands the projector may give, as its keys are the same", () => {
    const keys = [..."abcdefghijklmnopqrstuvwxyz0123456789", " ", "ArrowRight", "PageDown", "Enter", "Escape", "Backspace"];
    const all: Controls = { next: true, remoteNext: true, judge: true, pauseClock: true, resumeClock: true, closeQuestion: true, playVideo: true, goTo: true, restartQuestion: true, setJoker: true, readingDone: false, nextStep: "nextSlide" };
    const views = [viewWith(all), viewWith({ ...all, pauseClock: false }, true)];
    const given = new Set(views.flatMap((view) => keys.map((key) => commandForKey(key, view)?.type).filter((type) => type !== undefined)));
    expect([...given].filter((type) => !PROJECTOR_COMMANDS.includes(type))).toEqual([]);
    expect(given.size).toBe(PROJECTOR_COMMANDS.length);
  });

  it("presses exactly the buzzers the projector may press, with the big button", () => {
    const presses = [..."0123456789"].map((key) => commandForKey(key, viewWith({})));
    for (const press of presses) expect(press).toMatchObject({ type: "press", button: "big" });
    const buzzers = presses.map((press) => (press?.type === "press" ? press.buzzer : null));
    expect(buzzers.sort((a, b) => (a ?? 0) - (b ?? 0))).toEqual(KEY_BUZZERS);
  });

  it("toggles the leaderboard", () => {
    expect(commandForKey("s", viewWith({}))).toEqual({ type: "showLeaderboard", visible: true });
    expect(commandForKey("s", viewWith({}, true))).toEqual({ type: "showLeaderboard", visible: false });
    expect(commandForKey("x", viewWith({ next: true }))).toBeNull();
  });
});

describe("commandForAction", () => {
  it("does what the keyboard's key for the action does", () => {
    const all: Controls = { next: true, remoteNext: true, judge: true, pauseClock: true, resumeClock: false, closeQuestion: true, playVideo: true, goTo: true, restartQuestion: true, setJoker: true, readingDone: false, nextStep: "nextSlide" };
    const pairs = [
      ["next", " "],
      ["judgeRight", "y"],
      ["judgeWrong", "n"],
      ["clock", "p"],
      ["leaderboard", "s"],
    ] as const;
    for (const [action, key] of pairs) expect(commandForAction(action, viewWith(all))).toEqual(commandForKey(key, viewWith(all)));
  });

  it("goes back to the slide the show says Back goes to, if any", () => {
    expect(commandForAction("previous", { ...viewWith({ goTo: true }), previousSlide: 2 })).toEqual({ type: "goTo", slide: 2 });
    expect(commandForAction("previous", { ...viewWith({ goTo: true }), previousSlide: null })).toBeNull();
  });

  it("ends the question and plays the video only when the step takes it", () => {
    expect(commandForAction("closeQuestion", viewWith({ closeQuestion: true }))).toEqual({ type: "closeQuestion" });
    expect(commandForAction("closeQuestion", viewWith({ next: true }))).toBeNull();
    expect(commandForAction("playVideo", viewWith({ playVideo: true }))).toEqual({ type: "playVideo" });
    expect(commandForAction("playVideo", viewWith({ next: true }))).toBeNull();
    expect(commandForAction("projector", viewWith({ next: true, goTo: true }))).toBeNull();
  });
});

describe("commandForRemote", () => {
  const all: Controls = { next: true, remoteNext: true, judge: true, pauseClock: true, resumeClock: false, closeQuestion: true, playVideo: true, goTo: true, restartQuestion: true, setJoker: true, readingDone: false, nextStep: "nextSlide" };
  const clock = { limitMs: 30_000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 };
  const inStep = (step: "ready" | "running" | "closed"): GameView => {
    const phase: Phase = step === "running" ? { phase: "question", step, clock } : { phase: "question", step };
    // As `Game::controls` gives them: no Next from the remote while the question runs.
    return { ...viewWith({ ...all, remoteNext: step !== "running" }), phase };
  };

  it("does not end a running question with Next, so a second press by mistake cannot end it as it starts", () => {
    expect(commandForRemote("next", inStep("running"))).toBeNull();
    expect(commandForRemote("closeQuestion", inStep("running"))).toEqual({ type: "closeQuestion" });
    expect(commandForRemote("clock", inStep("running"))).toEqual({ type: "pauseClock" });
  });

  it("goes on with Next as the keyboard does once the question is not running", () => {
    for (const step of ["ready", "closed"] as const) expect(commandForRemote("next", inStep(step))).toEqual({ type: "next" });
    expect(commandForRemote("next", viewWith(all))).toEqual(commandForKey(" ", viewWith(all)));
  });
});

describe("keyHint", () => {
  it("writes keys as on the keyboard, in the quizmaster's language", () => {
    expect(keyHint(translator("cs"), KEYS.next)).toBe("Mezerník");
    expect(keyHint(translator("en"), KEYS.right)).toBe("Y");
  });
});

describe("keyCombo", () => {
  const press = (key: string, modifiers: Partial<Record<"ctrlKey" | "metaKey" | "altKey" | "shiftKey", boolean>>) => ({
    key,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    ...modifiers,
  });

  it("writes Ctrl for Ctrl, and for the Windows key, outside macOS", () => {
    expect(keyCombo(press("s", { ctrlKey: true }), false)).toBe("Ctrl+S");
    expect(keyCombo(press("z", { metaKey: true, shiftKey: true }), false)).toBe("Ctrl+Shift+Z");
    expect(keyCombo(press("F5", {}), false)).toBe("F5");
  });

  it("writes Ctrl for Cmd on macOS, and Control apart, so the caret keys of text fields stay theirs", () => {
    expect(keyCombo(press("s", { metaKey: true }), true)).toBe("Ctrl+S");
    expect(keyCombo(press("f", { ctrlKey: true }), true)).toBe("Control+F");
    expect(keyCombo(press("ArrowLeft", { altKey: true }), true)).toBe("Alt+ArrowLeft");
  });
});

describe("isPlainKey", () => {
  const press = (more: Partial<KeyboardEvent> = {}) =>
    ({ metaKey: false, ctrlKey: false, altKey: false, defaultPrevented: false, target: null, ...more }) as KeyboardEvent;

  it("takes a key with no Ctrl, Cmd or Alt, typed nowhere and taken by no control", () => {
    expect(isPlainKey(press())).toBe(true);
    // Shift is part of the key itself (M, ?), and a slider takes no typing.
    expect(isPlainKey(press({ shiftKey: true, target: { tagName: "INPUT", type: "range" } as unknown as EventTarget }))).toBe(true);
    const typing = { tagName: "TEXTAREA" } as unknown as EventTarget;
    for (const more of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { target: typing }, { defaultPrevented: true }]) {
      expect(isPlainKey(press(more)), JSON.stringify(more)).toBe(false);
    }
  });
});

describe("isProjectorCloseKey", () => {
  const key = (k: string, more: Partial<KeyboardEvent> = {}) =>
    ({ key: k, repeat: false, metaKey: false, ctrlKey: false, altKey: false, defaultPrevented: false, target: null, ...more }) as KeyboardEvent;

  it("closes the projector with M or Esc", () => {
    expect([key("m"), key("M"), key("Escape")].map(isProjectorCloseKey)).toEqual([true, true, true]);
    expect([key("n"), key(" "), key("Enter")].map(isProjectorCloseKey)).toEqual([false, false, false]);
  });

  it("leaves a held key, a key with a modifier, typing and a key a control took", () => {
    const typing = { tagName: "INPUT", type: "text" } as unknown as EventTarget;
    for (const more of [{ repeat: true }, { ctrlKey: true }, { metaKey: true }, { altKey: true }, { target: typing }, { defaultPrevented: true }]) {
      expect(isProjectorCloseKey(key("m", more)), JSON.stringify(more)).toBe(false);
    }
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
