// The buzzer receiver in the real app, simulated: the debug build talks to a
// fake Enjoy 303S receiver (crates/buzzers/src/fake_receiver.rs) that this
// test serves over TCP (`BUZZDOME_FAKE_RECEIVER`). Everything above the USB
// device is real: automatic detection, the receiver thread, the 303S
// protocol, the show. The test plugs the receiver in and out, presses
// buttons with the buzzers' own timers, and reads the colours the app sends.

import { createServer, type Server, type Socket } from "node:net";
import { createInterface } from "node:readline";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { Button, GameView, ReceiverInfo } from "../src/types";
import { App, waitFor } from "./harness";

/** The fake receiver's other end: commands to it, and the colours the app wrote. */
class FakeReceiver {
  private socket: Socket | null = null;
  /** Each buzzer's colour as the app last wrote it (a `Light` name, or null). */
  readonly lights = new Map<number, string | null>();
  readonly errors: string[] = [];
  private constructor(private readonly server: Server) {
    server.on("connection", (socket) => {
      this.socket = socket;
      createInterface({ input: socket }).on("line", (line) => {
        const message = JSON.parse(line) as { lights?: { buzzer: number; light: string | null }[]; error?: string };
        for (const lit of message.lights ?? []) this.lights.set(lit.buzzer, lit.light);
        if (message.error) this.errors.push(message.error);
      });
    });
  }

  static async open(): Promise<FakeReceiver> {
    const server = createServer();
    await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
    return new FakeReceiver(server);
  }

  get address(): string {
    const address = this.server.address();
    if (!address || typeof address !== "object") throw new Error("not listening");
    return `127.0.0.1:${address.port}`;
  }

  private async send(command: object): Promise<void> {
    // The app connects a moment after it starts, and again after a restart.
    const socket = await waitFor("the app to connect to the fake receiver", async () => (this.socket && !this.socket.destroyed ? this.socket : null));
    socket.write(`${JSON.stringify(command)}\n`);
  }

  plug(on: boolean): Promise<void> {
    return this.send({ plug: on });
  }

  /** A press of `button` on `buzzer`, `timer` being the buzzer's own response time. */
  press(buzzer: number, button: Button, timer = 0): Promise<void> {
    return this.send({ press: { buzzer, button, timer } });
  }

  close(): Promise<void> {
    this.socket?.destroy();
    return new Promise((done) => this.server.close(() => done()));
  }
}

let fake: FakeReceiver;
let app: App;

const info = () => app.invoke<ReceiverInfo>("receiver_info");
const connected = async () => (await info()).status?.kind === "connected";
const player = (view: GameView, buzzer: number) => view.players.find((p) => p.buzzer === buzzer);

beforeAll(async () => {
  fake = await FakeReceiver.open();
  // Only the debug build reads it (hardware.rs); the E2E tests run that one.
  process.env.BUZZDOME_FAKE_RECEIVER = fake.address;
  app = await App.launch();
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
  delete process.env.BUZZDOME_FAKE_RECEIVER;
  await fake?.close();
});

describe("a simulated buzzer receiver", () => {
  test("is found by itself once plugged in, and the Receiver tab says so", async () => {
    expect(await connected()).toBe(false);
    await fake.plug(true);
    await waitFor("the receiver found", connected, 15_000);
    const status = (await info()).status;
    expect(status).toMatchObject({ kind: "connected", device: { vendorId: 0x10c4, productId: 0x1819 } });
    // Set up, the receiver gets every buzzer dark.
    await waitFor("all 64 buzzers set to off", async () => fake.lights.size === 64 && [...fake.lights.values()].every((light) => light === "off"));
    await app.tab("tabReceiver");
    await app.seeText("Fake 303S receiver");
    await app.assertHealthy();
  });

  test("its buzzers join the show, answer, and are listed as heard", async () => {
    await app.press("demoQuiz");
    await app.waitView("the lobby", (v) => v.phase.phase === "lobby");
    for (const buzzer of [3, 7, 12]) await fake.press(buzzer, "big");
    await app.waitView("three players from the receiver", (v) => [3, 7, 12].every((buzzer) => player(v, buzzer)));
    expect((await info()).buzzers.map((seen) => seen.buzzer)).toEqual(expect.arrayContaining([3, 7, 12]));

    await app.step("next");
    await app.waitView("the first slide", (v) => v.slideIndex === 0);
    await app.step("next");
    await app.step("next");
    await app.waitView("the first question running", (v) => v.slideIndex === 1 && v.phase.phase === "question" && v.phase.step === "running");
    // Everyone may answer: their buzzers light up as ready.
    await waitFor("the players' buzzers ready", async () => [3, 7, 12].every((buzzer) => fake.lights.get(buzzer) === "ready"));
    await fake.press(3, "b");
    await fake.press(7, "a");
    await app.waitView("two answers", (v) => v.answeredCount === 2);
    await fake.press(12, "b");
    await app.waitView("the question closed by the last answer", (v) => v.phase.phase === "question" && v.phase.step === "closed");
    await app.step("next");
    // The reveal shows each player right or wrong on their buzzer.
    await waitFor("right and wrong on the buzzers", async () => fake.lights.get(3) === "correct" && fake.lights.get(7) === "wrong" && fake.lights.get(12) === "correct");
    expect(fake.errors).toEqual([]);
  });

  test("fastest finger goes by the buzzers' own timers, not by when the presses arrive", async () => {
    // Slide 8 of the demo quiz is a fastest-finger question for the big button.
    await app.tab("tabQuestions");
    await app.page.click(await app.$(".slide-grid tbody tr:nth-child(8) td:nth-child(1)"));
    await app.answerQuestions(true);
    await app.step("goToSelected");
    await app.waitView("slide 8", (v) => v.slideIndex === 7);
    await app.step("next");
    await app.waitView("the clock running", (v) => v.phase.phase === "question" && v.phase.step === "running");
    // Buzzer 7 pressed later by the clock of the room, but sooner by its own.
    await fake.press(12, "big", 400);
    await fake.press(7, "big", 150);
    const buzzed = await app.waitView("a buzz", (v) => v.phase.phase === "question" && v.phase.step === "buzzed");
    expect(buzzed.buzzed).toBe(7);
    await waitFor("buzzer 7 lit as first", async () => fake.lights.get(7) === "first");

    // Wrong: buzzer 7 shows it (until the next step, game.rs `light_of`), and its next press does nothing.
    await app.step("wrong");
    await app.waitView("the clock running again", (v) => v.phase.phase === "question" && v.phase.step === "running");
    await waitFor("buzzer 7 shown wrong", async () => fake.lights.get(7) === "wrong");
    await fake.press(7, "big", 10);
    await fake.press(3, "big", 500);
    const next = await app.waitView("the next buzz", (v) => v.phase.phase === "question" && v.phase.step === "buzzed");
    expect(next.buzzed).toBe(3);
    await app.step("right");
    await app.waitView("the answer revealed", (v) => v.phase.phase === "question" && v.phase.step === "revealed");
    await app.assertHealthy();
  });

  test("pulled out mid-show and plugged in again, it gets its colours back and plays on", async () => {
    const shown = new Map(fake.lights);
    await fake.plug(false);
    await waitFor("the receiver gone", async () => !(await connected()), 15_000);
    const players = (await app.view())!.players.length;
    // A press while it is out reaches no one.
    await fake.press(40, "big");
    fake.lights.clear();
    await fake.plug(true);
    await waitFor("the receiver back", connected, 15_000);
    // The receiver forgot its colours: the app sets them again.
    await waitFor("the colours back", async () => [3, 7, 12].every((buzzer) => fake.lights.get(buzzer) === shown.get(buzzer)), 15_000);
    expect((await app.view())!.players.length).toBe(players);
    await app.step("next");
    await app.waitView("the show going on", (v) => v.slideIndex > 7);
    expect(fake.errors).toEqual([]);
    await app.assertHealthy();
  });

  test("the Receiver tab's buttons all work with a receiver connected", async () => {
    await app.tab("tabReceiver");
    await app.clickEverything(".tab-body", ["exportLog"]);
    await app.assertHealthy();
  });
});
