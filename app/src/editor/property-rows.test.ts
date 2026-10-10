import { describe, expect, it } from "vitest";
import {
  alphabetical,
  canReset,
  commonRows,
  copiedValue,
  draggedValue,
  findMatch,
  pasteAction,
  resetValue,
  rowMatches,
  valueText,
  type CopiedValue,
  type PropertyRow,
  type RowValue,
} from "./property-rows";

/** A row value of any kind without its setter. */
type Unset<T> = T extends unknown ? Omit<T, "set"> : never;

/** A row whose value records what it is set to. */
function row(id: string, value?: Unset<RowValue>, label = id): PropertyRow & { setTo: unknown[] } {
  const setTo: unknown[] = [];
  const set = (v: unknown) => void setTo.push(v);
  return { id, label, description: `About ${label}`, control: null, value: value && ({ ...value, set } as RowValue), setTo };
}

/** Pastes as the menu does; whether it could. */
function paste(target: PropertyRow, copied: CopiedValue): boolean {
  const action = pasteAction(target, copied);
  action?.();
  return action !== null;
}

const time = (value: number) => row("answerTime", { kind: "number", value, initial: 20, min: 0, max: 3600, step: 1 });

describe("rows several slides share", () => {
  it("are the first slide's rows with a value that every slide has, mixed where they differ", () => {
    const sound = () => row("sound", { kind: "media", value: null, accepts: () => true });
    const first = [time(20), row("everyone", { kind: "flag", value: true }), sound(), row("notes")];
    const second = [time(30), row("everyone", { kind: "flag", value: true }), sound()];
    const third = [time(20), row("everyone", { kind: "flag", value: true })];
    expect(commonRows([first, second])).toEqual({ shown: new Set(["answerTime", "everyone", "sound"]), mixed: new Set(["answerTime"]) });
    expect(commonRows([first, second, third]).shown).toEqual(new Set(["answerTime", "everyone"]));
  });

  it("are all valued rows for one slide, none mixed", () => {
    expect(commonRows([[time(5), row("id")]])).toEqual({ shown: new Set(["answerTime"]), mixed: new Set() });
  });
});

describe("searching", () => {
  it("finds a match ignoring case and accents and keeps the text as written", () => {
    expect(findMatch("Čas na odpověď", "cas")).toEqual([
      { text: "Čas", match: true },
      { text: " na odpověď", match: false },
    ]);
    expect(findMatch("Answer time", "WER T")).toEqual([
      { text: "Ans", match: false },
      { text: "wer t", match: true },
      { text: "ime", match: false },
    ]);
    expect(findMatch("Odpověď", "ved")).toEqual([
      { text: "Odpo", match: false },
      { text: "věď", match: true },
    ]);
    expect(findMatch("Volume", "speed")).toBeNull();
    expect(findMatch("Volume", "  ")).toBeNull();
  });

  it("looks at the description too", () => {
    expect(rowMatches({ label: "Penalty", description: "Points taken for a wrong answer" }, "wrong")).toBe(true);
    expect(rowMatches({ label: "Penalty", description: "Points taken" }, "speed")).toBe(false);
  });
});

describe("dragging a number by its label", () => {
  const limits = { min: 0, max: 60, step: 1 };
  const plain = { shift: false, alt: false };

  it("moves a step per few pixels either way, within its limits", () => {
    expect(draggedValue(20, 0, limits, plain)).toBe(20);
    expect(draggedValue(20, 3, limits, plain)).toBe(20);
    expect(draggedValue(20, 8, limits, plain)).toBe(22);
    expect(draggedValue(20, -8, limits, plain)).toBe(18);
    expect(draggedValue(20, 1000, limits, plain)).toBe(60);
    expect(draggedValue(20, -1000, limits, plain)).toBe(0);
  });

  it("takes ten steps with Shift and goes slower with Alt", () => {
    expect(draggedValue(20, 8, limits, { shift: true, alt: false })).toBe(40);
    expect(draggedValue(20, 8, limits, { shift: false, alt: true })).toBe(20);
    expect(draggedValue(20, 32, limits, { shift: false, alt: true })).toBe(22);
  });

  it("stays on fractional steps without float noise", () => {
    expect(draggedValue(0.1, 8, { min: 0, max: 60, step: 0.1 }, plain)).toBe(0.3);
  });
});

describe("copying and pasting a value", () => {
  it("pastes a number into another number, within its limits and on its steps", () => {
    const copied = copiedValue(row("points", { kind: "number", value: 5000, min: -100000, max: 100000, step: 1 }));
    const speed = row("speed", { kind: "number", value: 100, min: 25, max: 250, step: 5 });
    expect(copied && paste(speed, copied)).toBe(true);
    expect(speed.setTo).toEqual([250]);
    const delay = row("delay", { kind: "number", value: 0, min: 0, max: 60, step: 0.1 });
    expect(paste(delay, { id: "x", kind: "number", value: 1.234 })).toBe(true);
    expect(delay.setTo).toEqual([1.2]);
  });

  it("pastes a choice only into the same property, and only one it offers", () => {
    const effect = row("effect", { kind: "choice", value: "none", options: ["none", "blur", "mosaic"] });
    expect(paste(effect, { id: "soundStart", kind: "choice", value: "blur" })).toBe(false);
    expect(paste(effect, { id: "effect", kind: "choice", value: "zoom" })).toBe(false);
    expect(paste(effect, { id: "effect", kind: "choice", value: "blur" })).toBe(true);
    expect(effect.setTo).toEqual(["blur"]);
  });

  it("refuses a value of another kind and a row without one", () => {
    const loop = row("loop", { kind: "flag", value: false });
    expect(paste(loop, { id: "speed", kind: "number", value: 1 })).toBe(false);
    expect(paste(row("id"), { id: "speed", kind: "number", value: 1 })).toBe(false);
    expect(paste(loop, { id: "muted", kind: "flag", value: true })).toBe(true);
    expect(loop.setTo).toEqual([true]);
  });

  it("pastes a file, or none, into a file row that takes its kind", () => {
    const picture = row("picture", { kind: "media", value: null, initial: null, accepts: (file: string) => file.endsWith(".png") });
    expect(paste(picture, { id: "sound", kind: "media", value: "media/song.mp3" })).toBe(false);
    expect(paste(picture, { id: "background", kind: "media", value: "media/flag.png" })).toBe(true);
    expect(paste(picture, { id: "sound", kind: "media", value: null })).toBe(true);
    expect(picture.setTo).toEqual(["media/flag.png", null]);
    expect(valueText({ id: "picture", kind: "media", value: null })).toBe("");
    expect(valueText({ id: "speed", kind: "number", value: 150 })).toBe("150");
  });
});

describe("resetting a value", () => {
  it("sets the default, and only where there is one to go back to", () => {
    const changed = time(45);
    expect(canReset(changed)).toBe(true);
    resetValue(changed);
    expect(changed.setTo).toEqual([20]);
    expect(canReset(time(20))).toBe(false);
    expect(canReset(row("type", { kind: "choice", value: "question", options: ["question"] }))).toBe(false);
    const picture = row("picture", { kind: "media", value: "media/a.png", initial: null, accepts: () => true });
    resetValue(picture);
    expect(picture.setTo).toEqual([null]);
  });
});

describe("the classic list", () => {
  it("sorts by label in the language's order and lists each property once", () => {
    const rows = [row("b", undefined, "Čas"), row("a", undefined, "Body"), row("c", undefined, "Cíl"), row("a", undefined, "Body")];
    expect(alphabetical([{ name: "Hra", rows }], "cs").map((r) => r.label)).toEqual(["Body", "Cíl", "Čas"]);
  });

  it("names the group of a label that several properties share", () => {
    const groups = [
      { name: "Video", rows: [row("video.volume", undefined, "Hlasitost"), row("video", undefined, "Video")] },
      { name: "Zvuk", rows: [row("sound.volume", undefined, "Hlasitost")] },
    ];
    expect(alphabetical(groups, "cs").map((r) => r.label)).toEqual(["Hlasitost (Video)", "Hlasitost (Zvuk)", "Video"]);
  });
});
