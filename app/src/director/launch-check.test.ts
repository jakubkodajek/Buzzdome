import { describe, expect, it } from "vitest";
import { NO_CONTROLS, previewView } from "../host/preview";
import { translator } from "../i18n";
import type { GameView, PlayerView, ReceiverInfo, ReceiverStatus } from "../types";
import { launchCheck, projectorScreen, type CheckItem, type LaunchFacts } from "./launch-check";

const t = translator("en");

const slide = { id: "b1", type: "billboard", title: "Welcome", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;

function lobby(buzzers: number[], teams: { name: string; buzzer: number | null }[] = []): GameView {
  const players = buzzers.map((buzzer) => ({ buzzer }) as PlayerView);
  return { ...previewView({ ...slide }, 0, 1, "Quiz"), phase: { phase: "lobby" }, controls: NO_CONTROLS, players, teams };
}

const device = { path: "/dev/hidraw0", vendorId: 0x10c4, productId: 0x1819, manufacturer: null, product: "CP2110", serialNumber: null, usagePage: 0, usage: 0, interfaceNumber: 0 };

function receiver(status: ReceiverStatus | null): ReceiverInfo {
  return {
    status,
    initError: status ? null : "no HID",
    settingsError: null,
    chosen: null,
    learnedButtons: 0,
    learn: { active: false, buzzer: null, button: null, samples: 0, canSave: false },
    recent: [],
    buzzers: [],
    remote: [],
    platform: "windows",
  };
}

const connected = receiver({ kind: "connected", device, protocol: { type: "enjoy303s", answered: true }, hasLights: true, learnable: false });

const GB = 1024 ** 3;

/** A show ready in every part. */
const ready: LaunchFacts = {
  receiver: connected,
  view: lobby([1, 2], [{ name: "Owls", buzzer: 1 }, { name: "Foxes", buzzer: 2 }]),
  projector: { onDirectorMonitor: false, monitors: 2, fullscreen: true },
  muted: null,
  sound: "heard",
  freeDiskBytes: 50 * GB,
};

/** The check's item for `part` with `facts` changed from a ready show. */
function item(part: CheckItem["part"], facts: Partial<LaunchFacts>): CheckItem | undefined {
  return launchCheck(t, { ...ready, ...facts }).find((found) => found.part === part);
}

describe("launchCheck", () => {
  it("lists every part in order, all ready, with nothing to fix", () => {
    const items = launchCheck(t, ready);
    expect(items.map((found) => [found.part, found.state, found.fixes])).toEqual([
      ["receiver", "ok", []],
      ["buzzers", "ok", []],
      ["projector", "ok", []],
      ["sound", "ok", ["playSound"]],
      ["disk", "ok", []],
    ]);
    expect(items[1].text).toBe("Every team has a buzzer. Buzzers registered: 2.");
    expect(items[4].text).toBe("50.0 GB free on the disk.");
  });

  it("warns that only the keyboard plays without a ready receiver, and calls a receiver error a problem", () => {
    const looking = item("receiver", { receiver: receiver({ kind: "looking" }) });
    expect(looking).toMatchObject({ state: "warn", fixes: ["receiverTab"] });
    expect(looking?.text).toContain("Until then only the keyboard and the on-screen buzzers play.");
    const notAnswering = receiver({ kind: "connected", device, protocol: { type: "enjoy303s", answered: false }, hasLights: true, learnable: false });
    expect(item("receiver", { receiver: notAnswering })?.state).toBe("warn");
    expect(item("receiver", { receiver: receiver(null) })).toMatchObject({ state: "warn", text: t("receiverUnavailable") });
    const failed = receiver({ kind: "error", problem: "connectionLost", detail: "unplugged" });
    expect(item("receiver", { receiver: failed })).toMatchObject({ state: "problem", fixes: ["receiverTab"] });
    expect(item("receiver", { receiver: null })).toMatchObject({ state: "todo", text: "Checking…", fixes: [] });
  });

  it("names the teams without a buzzer, the first five", () => {
    const names = ["A", "B", "C", "D", "E", "F", "G"];
    const teams = [{ name: "Owls", buzzer: 1 }, ...names.map((name) => ({ name, buzzer: null }))];
    expect(item("buzzers", { view: lobby([1], teams) })).toEqual({
      part: "buzzers",
      state: "warn",
      text: "Teams without a buzzer (7 of 8): A, B, C, D, E and 2 more. Buzzers registered: 1.",
      fixes: ["teamsTab"],
    });
  });

  it("without teams, counts the buzzers registered, and warns while there are none", () => {
    expect(item("buzzers", { view: lobby([1, 2, 3]) })).toMatchObject({ state: "ok", text: "Buzzers registered: 3." });
    expect(item("buzzers", { view: lobby([]) })?.state).toBe("warn");
  });

  it("wants the projector open, full screen, on a screen of its own", () => {
    expect(item("projector", { projector: null })).toMatchObject({ state: "problem", fixes: ["openProjector"] });
    expect(item("projector", { projector: { onDirectorMonitor: true, monitors: 1, fullscreen: false } })).toMatchObject({ state: "warn", fixes: [] });
    expect(item("projector", { projector: { onDirectorMonitor: true, monitors: 2, fullscreen: true } })).toMatchObject({ state: "warn", fixes: ["placeProjector"] });
    expect(item("projector", { projector: { onDirectorMonitor: false, monitors: 2, fullscreen: false } })).toMatchObject({ state: "warn", fixes: ["placeProjector"] });
    expect(item("projector", { projector: undefined })?.state).toBe("todo");
  });

  it("puts the projector full screen on the chosen screen only while the Director is not on it", () => {
    const monitor = (id: string, director: boolean) => ({ id, name: null, width: 1920, height: 1080, director });
    const monitors = [monitor("laptop", false), monitor("projector", true)];
    expect(projectorScreen(monitors, "laptop")).toBe("laptop");
    expect(projectorScreen(monitors, "projector")).toBeNull();
    expect(projectorScreen(monitors, "unplugged")).toBeNull();
    expect(projectorScreen(monitors, null)).toBeNull();
  });

  it("asks for a test sound until the quizmaster heard it, and says where the sound is turned down", () => {
    expect(item("sound", { sound: "notPlayed" })).toMatchObject({ state: "todo", fixes: ["playSound"] });
    expect(item("sound", { sound: "played" })).toMatchObject({ state: "todo", fixes: ["soundHeard", "playSound"] });
    expect(item("sound", { muted: "player" })).toMatchObject({ state: "warn", fixes: ["mediaTab"] });
    expect(item("sound", { muted: "quiz" })).toMatchObject({ state: "warn", fixes: [] });
  });

  it("warns below a gigabyte free, calls below 100 MB a problem, and leaves the disk out when the system does not say", () => {
    expect(item("disk", { freeDiskBytes: 600 * 1024 ** 2 })).toMatchObject({ state: "warn", text: "600 MB left on the disk. Enough for the show, but space is running low." });
    expect(item("disk", { freeDiskBytes: 50 * 1024 ** 2 })?.state).toBe("problem");
    expect(item("disk", { freeDiskBytes: undefined })?.state).toBe("todo");
    expect(item("disk", { freeDiskBytes: null })).toBeUndefined();
  });
});
