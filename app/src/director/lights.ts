// Changes to the venue's lights setup (`LightsSetup`, crates/lights) as the
// Lights tab makes them: pure functions, so a change never leaves a look
// naming a fixture or a stage naming a look that is gone. The backend
// checks every setup again when it is kept (`LightsSetup::check`).

import { UNIVERSE_SIZE } from "../bindings/LightsConstants";
import type { StringKey } from "../i18n";
import type { Channel, ChannelValue, Fixture, LightsSetup, Look, OutputChoice } from "../types";

/** The channel layouts most fixtures have, to start a fixture from. */
export const FIXTURE_PRESETS: { id: string; label: StringKey; channels: Channel[] }[] = [
  { id: "rgb", label: "lightsPresetRgb", channels: [{ kind: "red" }, { kind: "green" }, { kind: "blue" }] },
  { id: "rgbw", label: "lightsPresetRgbw", channels: [{ kind: "red" }, { kind: "green" }, { kind: "blue" }, { kind: "white" }] },
  { id: "dimmerRgb", label: "lightsPresetDimmerRgb", channels: [{ kind: "dimmer" }, { kind: "red" }, { kind: "green" }, { kind: "blue" }] },
  {
    id: "dimmerRgbStrobe",
    label: "lightsPresetDimmerRgbStrobe",
    channels: [{ kind: "dimmer" }, { kind: "red" }, { kind: "green" }, { kind: "blue" }, { kind: "strobe" }],
  },
  {
    id: "par7",
    label: "lightsPresetPar7",
    channels: [{ kind: "dimmer" }, { kind: "red" }, { kind: "green" }, { kind: "blue" }, { kind: "strobe" }, { kind: "fixed", value: 0 }, { kind: "unused" }],
  },
  {
    id: "dimmerRgbwStrobe",
    label: "lightsPresetDimmerRgbwStrobe",
    channels: [{ kind: "dimmer" }, { kind: "red" }, { kind: "green" }, { kind: "blue" }, { kind: "white" }, { kind: "strobe" }],
  },
  { id: "dimmer", label: "lightsPresetDimmer", channels: [{ kind: "dimmer" }] },
];

/** The kinds of channel, in the order the channel picker lists them. */
export const CHANNEL_KINDS: Channel["kind"][] = ["dimmer", "red", "green", "blue", "white", "strobe", "fixed", "unused"];

/** A channel of `kind`; a fixed one at 0. */
export function channelOf(kind: Channel["kind"]): Channel {
  return kind === "fixed" ? { kind, value: 0 } : { kind };
}

/** An id none of `items` has. */
function freshId(items: { id: number }[]): number {
  return items.reduce((max, item) => Math.max(max, item.id), 0) + 1;
}

/** The address of the channel after `fixture`'s last. */
function after(fixture: Fixture): number {
  return fixture.address + fixture.channels.length;
}

/** The first address after every fixture, where a new one fits; 1 if none fits there. */
export function freeAddress(fixtures: Fixture[], channels: number): number {
  const next = fixtures.reduce((last, fixture) => Math.max(last, after(fixture)), 1);
  return next + channels - 1 <= UNIVERSE_SIZE ? next : 1;
}

/** The fixtures whose channels share an address with another's, by id. */
export function overlapping(fixtures: Fixture[]): Set<number> {
  const owner = new Map<number, number>();
  const clash = new Set<number>();
  for (const fixture of fixtures) {
    for (let address = fixture.address; address < after(fixture); address++) {
      const other = owner.get(address);
      if (other !== undefined && other !== fixture.id) {
        clash.add(other);
        clash.add(fixture.id);
      }
      owner.set(address, fixture.id);
    }
  }
  return clash;
}

/** `setup` with a fixture of `channels` named `name`, after the others. */
export function addFixture(setup: LightsSetup, name: string, channels: Channel[]): LightsSetup {
  const fixture: Fixture = { id: freshId(setup.fixtures), name, address: freeAddress(setup.fixtures, channels.length), channels };
  return { ...setup, fixtures: [...setup.fixtures, fixture] };
}

/** `setup` with fixture `id` changed by `change`. */
export function changeFixture(setup: LightsSetup, id: number, change: Partial<Omit<Fixture, "id">>): LightsSetup {
  return { ...setup, fixtures: setup.fixtures.map((fixture) => (fixture.id === id ? { ...fixture, ...change } : fixture)) };
}

/** `setup` without fixture `id`, and its looks without it. */
export function removeFixture(setup: LightsSetup, id: number): LightsSetup {
  return {
    ...setup,
    fixtures: setup.fixtures.filter((fixture) => fixture.id !== id),
    looks: setup.looks.map((look) => ({ ...look, fixtures: look.fixtures.filter((fixture) => fixture !== id) })),
  };
}

/** `setup` with a new static look named `name`, in white on every fixture. */
export function addLook(setup: LightsSetup, name: string): LightsSetup {
  const look: Look = {
    id: freshId(setup.looks),
    name,
    colors: ["#FFFFFF"],
    intensity: 100,
    effect: "static",
    stepMs: 1000,
    fadeMs: 500,
    fixtures: [],
    strobe: 0,
    channels: [],
  };
  return { ...setup, looks: [...setup.looks, look] };
}

/** `setup` with look `id` changed by `change`. */
export function changeLook(setup: LightsSetup, id: number, change: Partial<Omit<Look, "id">>): LightsSetup {
  return { ...setup, looks: setup.looks.map((look) => (look.id === id ? { ...look, ...change } : look)) };
}

/** `setup` without look `id`; stages that took it keep the look before, and cues that flashed it no longer flash. */
export function removeLook(setup: LightsSetup, id: number): LightsSetup {
  return {
    ...setup,
    looks: setup.looks.filter((look) => look.id !== id),
    stages: Object.fromEntries(Object.entries(setup.stages).filter(([, look]) => look !== id)),
    flashes: Object.fromEntries(Object.entries(setup.flashes).filter(([, flash]) => flash?.look !== id)),
  };
}

/** Whether `choice` is the same output as `other`, settings and all. */
export function sameOutput(choice: OutputChoice, other: OutputChoice): boolean {
  return JSON.stringify(choice) === JSON.stringify(other);
}

/** The output of `kind` with what `before` had that it takes too, else its defaults. */
export function outputOfKind(kind: OutputChoice["kind"], before: OutputChoice): OutputChoice {
  const port = "port" in before ? before.port : "";
  switch (kind) {
    case "none":
    case "soundSwitch":
      return { kind };
    case "enttecPro":
    case "openDmx":
      return { kind, port };
    case "artNet":
      return { kind, host: "255.255.255.255", universe: 0 };
    case "sacn":
      return { kind, universe: 1, host: null };
  }
}

/**
 * The colour a fixture shows in `slots` (the universe, channel 1 first), as
 * CSS: its red, green and blue (white added), dimmed by its dimmer when it
 * has one; null for a fixture with no colour channels.
 */
export function fixtureColor(fixture: Fixture, slots: number[]): string | null {
  let [r, g, b, w, dimmer, colored] = [0, 0, 0, 0, 255, false];
  fixture.channels.forEach((channel, i) => {
    const value = slots[fixture.address - 1 + i] ?? 0;
    if (channel.kind === "red") [r, colored] = [value, true];
    if (channel.kind === "green") [g, colored] = [value, true];
    if (channel.kind === "blue") [b, colored] = [value, true];
    if (channel.kind === "white") [w, colored] = [value, true];
    if (channel.kind === "dimmer") dimmer = value;
  });
  if (!colored) return fixture.channels.some((c) => c.kind === "dimmer") ? `rgb(${dimmer}, ${dimmer}, ${dimmer})` : null;
  const level = (value: number) => Math.round((Math.min(255, value + w) * dimmer) / 255);
  return `rgb(${level(r)}, ${level(g)}, ${level(b)})`;
}

/** A look's channels set by address, as typed: "100=255, 101=0". */
export function formatChannelValues(channels: ChannelValue[]): string {
  return channels.map(({ address, value }) => `${address}=${value}`).join(", ");
}

/** Reads channels typed as `formatChannelValues` writes them; null when a part is no address=value in range. */
export function parseChannelValues(text: string): ChannelValue[] | null {
  const parts = text.split(/[,;\s]+/).filter((part) => part !== "");
  const channels: ChannelValue[] = [];
  for (const part of parts) {
    const match = /^(\d+)=(\d+)$/.exec(part);
    if (!match) return null;
    const [address, value] = [Number(match[1]), Number(match[2])];
    if (address < 1 || address > UNIVERSE_SIZE || value > 255) return null;
    channels.push({ address, value });
  }
  return channels;
}
