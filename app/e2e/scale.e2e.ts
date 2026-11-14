// A full show at the size of a real evening: 24 teams on buzzers of the
// simulated Enjoy 303S receiver (receiver.e2e.ts has the fake's wire), a
// quiz of 30 questions, every press of a round sent in one write so the
// receiver takes them at the same moment, and a crash halfway through.
// Each step checks every team's score and every buzzer's light against
// what the rules give, so a press lost, counted twice or ordered by its
// arrival rather than its buzzer's timer fails the test.

import { writeFileSync } from "node:fs";
import { createServer, type Server, type Socket } from "node:net";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { Button, GameView, ReceiverInfo, ShowResults } from "../src/types";
import { App, dataFolder, sleep, t, waitFor } from "./harness";

/** The teams of the evening: a colleague's show is planned for about 20. */
const TEAMS = 24;
const BUZZERS = Array.from({ length: TEAMS }, (_, i) => i + 1);
const QUESTIONS = 30;
/** The question after which the app is killed. */
const CRASH_AFTER = 14;
const EVERYONE_POINTS = 100;
const FASTEST_POINTS = 150;
const FASTEST_PENALTY = 30;
const ANSWERS: Button[] = ["a", "b", "c", "d"];
/** How long the presses to a step may take to show with 24 teams: the same as one, with room for a slow machine. */
const STEP_MS = 15_000;

/** A fixed sequence of numbers, so a failure plays again the same way. */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let x = Math.imul(state ^ (state >>> 15), 1 | state);
    x ^= x + Math.imul(x ^ (x >>> 7), 61 | x);
    return ((x ^ (x >>> 14)) >>> 0) / 4_294_967_296;
  };
}
const rng = random(Number(process.env.SCALE_SEED ?? 20261214));
const pick = <T>(items: readonly T[]): T => items[Math.floor(rng() * items.length)];
const shuffled = <T>(items: readonly T[]): T[] => {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
};

/** Every third question is a fastest finger for the big button; the rest are everyone-answers A-D. */
const fastest = (question: number) => question % 3 === 2;
const correct = Array.from({ length: QUESTIONS }, () => Math.floor(rng() * 4));

function quiz(): object {
  const slides: object[] = [{ id: "round", type: "billboard", title: "Velké kolo", text: "24 týmů" }];
  for (let q = 0; q < QUESTIONS; q++) {
    slides.push(
      fastest(q)
        ? { id: `q${q}`, type: "question", mode: "fastestFinger", text: `Rychlá otázka ${q + 1}`, timeLimitS: 60, points: FASTEST_POINTS, penalty: FASTEST_PENALTY }
        : {
            id: `q${q}`,
            type: "question",
            mode: "everyone",
            text: `Otázka ${q + 1}`,
            answers: ["Jedna", "Dva", "Tři", "Čtyři"],
            correct: [{ answer: correct[q], share: 100 }],
            timeLimitS: 60,
            points: EVERYONE_POINTS,
          },
    );
  }
  slides.push({ id: "final", type: "final", title: "Vítězové" });
  return { schemaVersion: 5, title: "Velký kvíz", slides };
}

/** The fake receiver's other end (as in receiver.e2e.ts): presses to it, the lights the app wrote. */
class FakeReceiver {
  private socket: Socket | null = null;
  readonly lights = new Map<number, string | null>();
  readonly errors: string[] = [];
  private constructor(private readonly server: Server) {
    server.on("connection", (socket) => {
      this.socket = socket;
      socket.on("error", () => socket.destroy());
      const lines = createInterface({ input: socket });
      lines.on("error", () => lines.close());
      lines.on("line", (line) => {
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

  private async send(...commands: object[]): Promise<void> {
    const socket = await waitFor("the app to connect to the fake receiver", async () => (this.socket && !this.socket.destroyed ? this.socket : null));
    socket.write(commands.map((command) => `${JSON.stringify(command)}\n`).join(""));
  }

  plug(on: boolean): Promise<void> {
    return this.send({ plug: on });
  }

  /** Presses made at the same moment, in the order the radio delivers them. */
  pressTogether(presses: { buzzer: number; button: Button; timer: number }[]): Promise<void> {
    return this.send(...presses.map((press) => ({ press })));
  }

  close(): Promise<void> {
    this.socket?.destroy();
    return new Promise((done) => this.server.close(() => done()));
  }
}

const data = dataFolder();
let fake: FakeReceiver;
let app: App;
/** Each team's score as the rules give it, kept by the test. */
const expected = new Map<number, number>(BUZZERS.map((buzzer) => [buzzer, 0]));

const step = (view: GameView) => (view.phase.phase === "question" ? `question:${view.phase.step}` : view.phase.phase);
const scores = (view: GameView) => new Map(view.players.map((p) => [p.buzzer, p.score]));
const connected = async () => (await app.invoke<ReceiverInfo>("receiver_info")).status?.kind === "connected";
const lightsAre = (what: string, light: (buzzer: number) => string) =>
  waitFor(what, async () => BUZZERS.every((buzzer) => fake.lights.get(buzzer) === light(buzzer)), STEP_MS).catch((error) => {
    const wrong = BUZZERS.filter((buzzer) => fake.lights.get(buzzer) !== light(buzzer)).map((b) => `${b}:${fake.lights.get(b)}≠${light(b)}`);
    throw new Error(`${error}: ${wrong.join(", ")}`);
  });

function expectScores(view: GameView): void {
  expect(Object.fromEntries(scores(view))).toEqual(Object.fromEntries(expected));
}

/** Next, and the question `q` running, its buzzers lit ready. */
async function toQuestion(q: number): Promise<void> {
  await app.step("next");
  await app.waitView(`question ${q + 1} running`, (v) => v.slideIndex === q + 1 && step(v) === "question:running", STEP_MS);
  await lightsAre(`question ${q + 1}: every buzzer ready`, () => "ready");
}

/** Everyone answers at once, in a shuffled order; the last answer closes the question, Next reveals it. */
async function everyoneAnswers(q: number): Promise<void> {
  const given = new Map(BUZZERS.map((buzzer) => [buzzer, pick(ANSWERS)]));
  // Each buzzer's own timer, as the real ones send it: a press with the same timer as one of the last
  // two seconds is that press sent again (edge.rs `TIMED_REPEAT_MS`).
  const timers = new Map(BUZZERS.map((buzzer) => [buzzer, 200 + Math.floor(rng() * 1_300)]));
  await sleep(1_600);
  await fake.pressTogether(shuffled(BUZZERS).map((buzzer) => ({ buzzer, button: given.get(buzzer)!, timer: timers.get(buzzer)! })));
  const closed = await app.waitView(`question ${q + 1} closed by the last of ${TEAMS} answers`, (v) => step(v) === "question:closed", STEP_MS).catch(async (error) => {
    const view = (await app.view())!;
    const answered = view.players.filter((p) => p.answered).map((p) => p.buzzer);
    throw new Error(`${error}: ${view.answeredCount} answered (${answered.join(", ")}), step ${step(view)}`);
  });
  expect(closed.answeredCount).toBe(TEAMS);
  await app.step("next");
  const revealed = await app.waitView(`question ${q + 1} revealed`, (v) => step(v) === "question:revealed", STEP_MS);
  const right = (buzzer: number) => given.get(buzzer) === ANSWERS[correct[q]];
  for (const buzzer of BUZZERS) if (right(buzzer)) expected.set(buzzer, expected.get(buzzer)! + EVERYONE_POINTS);
  expectScores(revealed);
  await lightsAre(`question ${q + 1}: right and wrong on every buzzer`, (buzzer) => (right(buzzer) ? "correct" : "wrong"));
}

/**
 * Every team buzzes within a few milliseconds, delivered in a shuffled order:
 * the queue follows the buzzers' own timers. The first two are judged wrong,
 * the third right.
 */
async function everyoneBuzzes(q: number): Promise<void> {
  // Distinct timers 3 ms apart, as the radio's slots would give them.
  const timers = new Map(shuffled(BUZZERS).map((buzzer, i) => [buzzer, 40 + 3 * i]));
  const order = [...BUZZERS].sort((a, b) => timers.get(a)! - timers.get(b)!);
  // The app drops a buzz whose timer is longer than the round has run (receiver.rs `stale_buzzes`).
  await sleep(Math.max(...timers.values()) + 150);
  await fake.pressTogether(shuffled(BUZZERS).map((buzzer) => ({ buzzer, button: "big" as Button, timer: timers.get(buzzer)! })));
  const buzzed = await app.waitView(`question ${q + 1}: the fastest buzz`, (v) => step(v) === "question:buzzed" && v.buzzes.length === TEAMS, STEP_MS);
  expect(buzzed.buzzed).toBe(order[0]);
  expect(buzzed.buzzes.map((b) => b.buzzer)).toEqual(order);
  expect(buzzed.buzzes.map((b) => b.state)).toEqual(["answering", ...Array(TEAMS - 1).fill("waiting")]);
  await lightsAre(`question ${q + 1}: first and waiting`, (buzzer) => (buzzer === order[0] ? "first" : "waiting"));

  for (const k of [0, 1]) {
    await app.step("wrong");
    expected.set(order[k], expected.get(order[k])! - FASTEST_PENALTY);
    const next = await app.waitView(`question ${q + 1}: buzzer ${order[k + 1]}'s turn`, (v) => step(v) === "question:buzzed" && v.buzzed === order[k + 1], STEP_MS);
    expectScores(next);
  }
  await app.step("right");
  expected.set(order[2], expected.get(order[2])! + FASTEST_POINTS);
  const revealed = await app.waitView(`question ${q + 1} revealed`, (v) => step(v) === "question:revealed", STEP_MS);
  expectScores(revealed);
  await waitFor(`question ${q + 1}: the winner's buzzer correct`, async () => fake.lights.get(order[2]) === "correct", STEP_MS);
}

beforeAll(async () => {
  fake = await FakeReceiver.open();
  process.env.BUZZDOME_FAKE_RECEIVER = fake.address;
  app = await App.launch(data);
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
  delete process.env.BUZZDOME_FAKE_RECEIVER;
  await fake?.close();
});

describe(`a show of ${QUESTIONS} questions with ${TEAMS} teams on the receiver`, () => {
  // The quiz opens through the system's picker (`App.pickFile`), driven through the X server.
  test.skipIf(process.platform !== "linux")(
    "every press counts once, in the buzzers' order, through a crash and to the results",
    async () => {
      await fake.plug(true);
      await waitFor("the receiver found", connected, 15_000);
      const file = join(app.data, "big-quiz.json");
      writeFileSync(file, JSON.stringify(quiz()));
      await app.press("openQuiz");
      await app.pickFile(file);
      await app.waitView("the lobby", (v) => v.phase.phase === "lobby");

      // All teams register with one press each, at the same moment.
      await fake.pressTogether(shuffled(BUZZERS).map((buzzer) => ({ buzzer, button: "big" as Button, timer: 0 })));
      const lobby = await app.waitView(`${TEAMS} teams`, (v) => v.players.length === TEAMS, STEP_MS);
      expect(lobby.players.map((p) => p.buzzer).sort((a, b) => a - b)).toEqual(BUZZERS);

      await app.step("next");
      await app.waitView("the first slide", (v) => v.slideIndex === 0, STEP_MS);
      for (let q = 0; q < QUESTIONS; q++) {
        await toQuestion(q);
        if (fastest(q)) await everyoneBuzzes(q);
        else await everyoneAnswers(q);

        if (q === CRASH_AFTER) {
          const before = (await app.view())!;
          await app.crash();
          app = await App.launch(data);
          await app.seeText(t("resume"));
          // The fake receiver keeps no state of its own across the app's restart: it is plugged in again.
          await fake.plug(true);
          await app.press("resume");
          const resumed = await app.waitView("the resumed show", (v) => v.slideIndex === before.slideIndex, STEP_MS);
          expect(step(resumed)).toBe("question:revealed");
          expect(resumed.players.map(({ buzzer, name, enabled }) => ({ buzzer, name, enabled }))).toEqual(
            before.players.map(({ buzzer, name, enabled }) => ({ buzzer, name, enabled })),
          );
          expectScores(resumed);
          await waitFor("the receiver back", connected, 15_000);
        }
      }

      await app.step("next");
      await app.waitView("the final", (v) => v.slideIndex === QUESTIONS + 1, STEP_MS);
      const results = (await app.invoke<ShowResults | null>("show_results"))!;
      expect(results.players).toHaveLength(TEAMS);
      for (const p of results.players) {
        expect(p.score).toBe(expected.get(p.buzzer));
        expect(p.rounds.reduce((sum, points) => sum + points, p.adjusted)).toBe(p.score);
      }
      // A higher score never has a lower place (an equal score goes by the answer times, `Game::ranking`).
      for (const p of results.players) for (const o of results.players) if (p.score > o.score) expect(p.rank!).toBeLessThan(o.rank!);
      // A spare buzzer pressed by mistake joins as a team: the Director says so at once.
      await fake.pressTogether([{ buzzer: 40, button: "big", timer: 0 }]);
      await app.waitView("the spare buzzer joined", (v) => v.players.length === TEAMS + 1, STEP_MS);
      await app.seeText(t("lateJoin", { buzzers: "40" }));
      expect(fake.errors).toEqual([]);
      await app.assertHealthy();
    },
    // 30 questions with step pauses and a restart: a few minutes.
    900_000,
  );
});
