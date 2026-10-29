// The stage lights in the real app (Director › Lights): a setup made in the
// tab drives a virtual Art-Net node on this machine (the test listens on
// Art-Net's UDP port), looks are held and follow the show, the show's
// messages arrive over OSC, a fixture comes from the fixture library, every
// button of the tab is clicked, and the setup survives a restart.

import { createSocket, type Socket } from "node:dgram";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { StringKey } from "../src/i18n";
import { App, dataFolder, t, waitFor } from "./harness";
import { Key } from "./webdriver";

/** Art-Net's UDP port (crates/lights/src/dmx/artnet.rs). */
const ARTNET_PORT = 6454;
/** Where ArtDmx carries the first channel's value (Art-Net 4: an 18-byte header). */
const ARTDMX_DATA = 18;
/** The OSC port the tab offers when OSC is switched on. */
const OSC_PORT = 7700;

/**
 * Buttons of the Lights tab the crawl leaves
 * alone: updating the fixture library downloads it from the internet, each
 * Add makes more buttons to click, and a colour swatch opens a picker over
 * the tab.
 */
const NOT_CLICKED: (StringKey | { text: string })[] = [
  "lightsLibraryUpdate",
  "lightsLibraryReset",
  "lightsAddFixture",
  "lightsAddLook",
  "lightsAddColor",
  "lightsAddChannel",
  { text: t("lightsColor", { n: "" }) },
];

const data = dataFolder();
let app: App;

/** A UDP port on this machine that keeps what arrives. */
class Listener {
  readonly packets: Buffer[] = [];
  private constructor(private readonly socket: Socket) {
    socket.on("message", (packet) => {
      this.packets.push(packet);
      if (this.packets.length > 2_000) this.packets.splice(0, 1_000);
    });
  }

  static open(port: number): Promise<Listener> {
    const socket = createSocket({ type: "udp4", reuseAddr: true });
    return new Promise((done, fail) => {
      socket.once("error", fail);
      socket.bind(port, "127.0.0.1", () => done(new Listener(socket)));
    });
  }

  close(): Promise<void> {
    return new Promise((done) => this.socket.close(() => done()));
  }
}

let artNet: Listener;
let osc: Listener;

/** The channel values (1-based `from` to `to`) of the last ArtDmx packet, once one arrived. */
function dmx(from: number, to: number): number[] | null {
  const packet = [...artNet.packets].reverse().find((p) => p.subarray(0, 8).toString("latin1") === "Art-Net\0" && p.readUInt16LE(8) === 0x5000);
  return packet ? [...packet.subarray(ARTDMX_DATA + from - 1, ARTDMX_DATA + to)] : null;
}

/** Waits until the node receives `values` on channels 1 to 3. */
async function waitDmx(what: string, values: number[]): Promise<void> {
  await waitFor(`the Art-Net node to get ${what} (${values.join(",")})`, async () => JSON.stringify(dmx(1, values.length)) === JSON.stringify(values), 15_000);
}

/** The OSC addresses received so far, in order. */
function oscAddresses(): string[] {
  return osc.packets.map((packet) => packet.subarray(0, packet.indexOf(0)).toString("latin1"));
}

/** Chooses `value` in the first shown drop-down labelled `label`, as a click on its option. */
async function choose(label: string, value: string | number): Promise<void> {
  await app.$(`select[aria-label="${label}"]`);
  await app.page.click(await app.$(`select[aria-label="${label}"] option[value="${value}"]`));
}

/** Types `text` into the first shown field labelled `label`, then Enter. */
async function fill(label: string, text: string): Promise<void> {
  const field = await app.$(`input[aria-label="${label}"]`);
  await app.page.clear(field);
  await app.page.type(field, text + Key.Enter);
}

/** Channels 1 to 3 as the app has them now. */
const universe = () => app.invoke<number[]>("lights_universe").then((values) => values.slice(0, 3));

/** Each look's Hold button, in the looks' order. */
async function holdButtons() {
  const shown = [];
  for (const button of await app.page.findAllXPath(`//button[normalize-space(.) = "${t("lightsHoldLook")}"]`)) if (await app.page.displayed(button)) shown.push(button);
  return shown;
}

/** The lights setup the backend keeps now. */
function setup() {
  return app.invoke<{ setup: { fixtures: { name: string }[]; looks: { id: number; name: string; intensity: number }[]; output: { kind: string; host?: string } } }>("lights_info").then((info) => info.setup);
}

beforeAll(async () => {
  artNet = await Listener.open(ARTNET_PORT);
  osc = await Listener.open(OSC_PORT);
  app = await App.launch(data);
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
  await artNet?.close();
  await osc?.close();
});

describe("the stage lights", () => {
  test("a setup made in the tab drives an Art-Net node, and a held look lights it", async () => {
    await app.tab("tabLights");
    await app.seeText(t("lightsOff"));
    await choose(t("lightsInterface"), "artNet");
    await fill(t("lightsHost"), "127.0.0.1");
    await waitFor("the Art-Net output kept", async () => (await setup()).output.kind === "artNet" && (await setup()).output.host === "127.0.0.1");
    await app.seeText(t("lightsConnected", { name: "" }).trim());
    // Nothing lit yet: the node gets a dark universe.
    await waitDmx("a dark universe", [0, 0, 0]);

    // The app starts with looks for each moment of the show and no fixtures.
    const before = await setup();
    await choose(t("lightsAddFixture"), "rgb");
    await app.press("lightsAddFixture");
    await app.press("lightsAddLook");
    await waitFor("the fixture and the look kept", async () => {
      const kept = await setup();
      return kept.fixtures.length === before.fixtures.length + 1 && kept.looks.length === before.looks.length + 1;
    });
    // A new look is white on every fixture, at full intensity; held, it lights the node.
    await holdButtons().then((buttons) => app.page.click(buttons.at(-1)!));
    await waitDmx("the held white look", [255, 255, 255]);
    expect(await universe()).toEqual([255, 255, 255]);

    await app.press("lightsBlackout");
    await waitDmx("the blackout", [0, 0, 0]);
    await app.press("lightsBlackoutOn");
    await waitDmx("the look after the blackout", [255, 255, 255]);
    // Released, the lights go back to the look the show's moment takes.
    await app.press("lightsRelease");
    await waitFor("the show's own look again", async () => {
      const values = dmx(1, 3);
      return values !== null && JSON.stringify(values) !== "[255,255,255]" && JSON.stringify(values) === JSON.stringify(await universe());
    }, 15_000);
    await app.assertHealthy();
  });

  test("the looks follow the show, and the show's messages go out over OSC", async () => {
    // A second look at half intensity for the slides, the first for the lobby.
    const count = (await setup()).looks.length;
    await app.press("lightsAddLook");
    const intensity = (await app.page.findAll(`input[type=number][aria-label="${t("lightsIntensity")}"]`)).at(-1)!;
    await app.page.clear(intensity);
    await app.page.type(intensity, "50" + Key.Enter);
    const [white, half] = await waitFor("the half-bright look kept", async () => {
      const kept = await setup();
      return kept.looks.length === count + 1 && kept.looks.at(-1)!.intensity === 50 ? kept.looks.slice(-2) : null;
    });
    await choose(t("lightsStageLobby"), white.id);
    await choose(t("lightsStageBillboard", { s: 0 }), half.id);
    await app.page.click(await app.$(`button[role=switch][aria-label="${t("lightsOsc")}"]`));
    await waitFor("the stages and OSC kept", async () => {
      const info = await app.invoke<{ setup: { stages: Record<string, number>; messages: { osc: unknown } } }>("lights_info");
      return info.setup.stages.lobby === white.id && info.setup.stages.billboard === half.id && info.setup.messages.osc !== null;
    });

    await app.press("demoQuiz");
    await app.waitView("the lobby", (v) => v.phase.phase === "lobby");
    await waitDmx("the lobby's look", [255, 255, 255]);
    await waitFor("the show's start over OSC", async () => oscAddresses().includes("/buzzdome/show/start") && oscAddresses().includes("/buzzdome/stage/lobby"));

    await app.step("next");
    await app.waitView("the first slide", (v) => v.phase.phase === "billboard");
    const bright = await waitFor("the slide's half-bright look", async () => {
      const values = dmx(1, 3);
      return values && values.every((v) => v > 100 && v < 160) && values[0] === values[1] && values[1] === values[2] ? values : null;
    }, 15_000);
    expect(bright[0]).toBeGreaterThan(100);
    await waitFor("the slide over OSC", async () => oscAddresses().includes("/buzzdome/stage/billboard") && oscAddresses().includes("/buzzdome/slide"));

    await app.answerQuestions(true);
    await app.press("endShow");
    await waitFor("the show ended", async () => (await app.view()) === null);
    await waitFor("the show's end over OSC", async () => oscAddresses().includes("/buzzdome/show/end"));
    await app.assertHealthy();
  });

  test("a fixture comes from the fixture library with the mode chosen", async () => {
    await app.tab("tabLights");
    const before = (await setup()).fixtures.length;
    const search = await app.$(`input[type=search][aria-label="${t("lightsLibrarySearch")}"]`);
    await app.page.type(search, "par");
    const model = await app.$(".lights-library .lights-found li button");
    const name = (await app.page.text(model)).trim();
    expect(name.length).toBeGreaterThan(0);
    await app.page.click(model);
    // The model's add button is the one next to its mode, in the library row.
    await app.page.click(await app.$(".lights-library .row button"));
    const added = await waitFor("the library fixture kept", async () => {
      const kept = await setup();
      return kept.fixtures.length === before + 1 ? kept.fixtures.at(-1)! : null;
    });
    expect(name).toContain(added.name);
    await app.assertHealthy();
  });

  // A button or more for each look, and every click checked: slow on a busy runner.
  test("every button of the Lights tab works", { timeout: 600_000 }, async () => {
    await app.tab("tabLights");
    await app.clickEverything(".tab-body", NOT_CLICKED);
    await app.assertHealthy();
  });

  test("the setup survives a restart", async () => {
    await app.press("lightsRelease").catch(() => undefined);
    const kept = await setup();
    await app.quit();
    app = await App.launch(data);
    const again = await setup();
    expect(again.output).toEqual(kept.output);
    expect(again.fixtures.map((f) => f.name)).toEqual(kept.fixtures.map((f) => f.name));
    expect(again.looks.map((l) => l.name)).toEqual(kept.looks.map((l) => l.name));
    await app.tab("tabLights");
    await app.seeText(t("lightsConnected", { name: "" }).trim());
    await app.assertHealthy();
  });
});
