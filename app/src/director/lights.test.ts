import { describe, expect, it } from "vitest";
import type { Fixture, LightsSetup } from "../types";
import {
  addFixture,
  channelOf,
  addLook,
  changeFixture,
  fixtureColor,
  FIXTURE_PRESETS,
  formatChannelValues,
  freeAddress,
  outputOfKind,
  overlapping,
  parseChannelValues,
  removeFixture,
  removeLook,
} from "./lights";

const rgb = FIXTURE_PRESETS.find((preset) => preset.id === "rgb")!.channels;
const dimmerRgb = FIXTURE_PRESETS.find((preset) => preset.id === "dimmerRgb")!.channels;

const empty: LightsSetup = {
  output: { kind: "none" },
  fixtures: [],
  looks: [],
  stages: {},
  flashes: {},
  master: 100,
  messages: { osc: null, midi: null },
};

const fixture = (id: number, address: number, channels = rgb): Fixture => ({ id, name: `F${id}`, address, channels, labels: [] });

describe("lights setup", () => {
  it("puts a new fixture after the others, with an id of its own", () => {
    const one = addFixture(empty, "Par", rgb);
    const two = addFixture(one, "Par", dimmerRgb);
    expect(two.fixtures.map((f) => [f.id, f.address])).toEqual([
      [1, 1],
      [2, 4],
    ]);
    expect(two.fixtures[0].labels).toEqual([]);
    const named = addFixture(empty, "Par", rgb, ["Red", "Green", "Blue"]);
    expect(named.fixtures[0].labels).toEqual(["Red", "Green", "Blue"]);
  });

  it("starts a fixture that no longer fits after the others at channel 1", () => {
    expect(freeAddress([fixture(1, 510)], 3)).toBe(1);
    expect(freeAddress([fixture(1, 507)], 3)).toBe(510);
  });

  it("finds fixtures that share a channel", () => {
    expect(overlapping([fixture(1, 1), fixture(2, 3), fixture(3, 10)])).toEqual(new Set([1, 2]));
    expect(overlapping([fixture(1, 1), fixture(2, 4)]).size).toBe(0);
  });

  it("takes a removed fixture out of the looks", () => {
    let setup = addLook(addFixture(addFixture(empty, "A", rgb), "B", rgb), "Look");
    setup = { ...setup, looks: [{ ...setup.looks[0], fixtures: [1, 2] }] };
    expect(removeFixture(setup, 1).looks[0].fixtures).toEqual([2]);
    expect(changeFixture(setup, 2, { address: 100 }).fixtures[1].address).toBe(100);
  });

  it("takes a removed look out of the stages and flashes", () => {
    let setup = addLook(addLook(empty, "A"), "B");
    setup = { ...setup, stages: { lobby: 1, answering: 2 }, flashes: { correct: { look: 1, ms: 1000 }, wrong: { look: 2, ms: 1000 } } };
    const removed = removeLook(setup, 1);
    expect(removed.looks.map((look) => look.id)).toEqual([2]);
    expect(removed.stages).toEqual({ answering: 2 });
    expect(removed.flashes).toEqual({ wrong: { look: 2, ms: 1000 } });
  });

  it("keeps a port when the output changes between the two Enttec kinds", () => {
    expect(outputOfKind("openDmx", { kind: "enttecPro", port: "COM3" })).toEqual({ kind: "openDmx", port: "COM3" });
    expect(outputOfKind("sacn", { kind: "none" })).toEqual({ kind: "sacn", universe: 1, host: null });
  });

  it("shows a fixture's colour dimmed by its dimmer, white added", () => {
    const slots = new Array<number>(512).fill(0);
    slots.splice(0, 4, 128, 255, 0, 0);
    expect(fixtureColor(fixture(1, 1, dimmerRgb), slots)).toBe("rgb(128, 0, 0)");
    slots.splice(9, 4, 0, 100, 50, 50);
    expect(fixtureColor(fixture(2, 10, [...rgb, { kind: "white" }]), slots)).toBe("rgb(50, 150, 100)");
    expect(fixtureColor(fixture(3, 20, [{ kind: "strobe" }]), slots)).toBeNull();
  });

  it("reads channels typed as address=value and refuses anything else", () => {
    expect(parseChannelValues("100=255, 101=0;7=1")).toEqual([
      { address: 100, value: 255 },
      { address: 101, value: 0 },
      { address: 7, value: 1 },
    ]);
    expect(parseChannelValues("  ")).toEqual([]);
    for (const wrong of ["100", "0=1", "513=1", "1=256", "a=1"]) expect(parseChannelValues(wrong)).toBeNull();
    expect(formatChannelValues([{ address: 3, value: 9 }, { address: 4, value: 0 }])).toBe("3=9, 4=0");
  });
});

describe("channels", () => {
  it("starts a shutter open at 255 and closed at 0, a dimmer up to 255, and a fixed one at 0", () => {
    expect(channelOf("shutter")).toEqual({ kind: "shutter", open: 255, closed: 0 });
    expect(channelOf("dimmerUpTo")).toEqual({ kind: "dimmerUpTo", max: 255 });
    expect(channelOf("fixed")).toEqual({ kind: "fixed", value: 0 });
    expect(channelOf("red")).toEqual({ kind: "red" });
  });
});
