import { describe, expect, it } from "vitest";
import { createStepGuard, STEP_PAUSE_MS } from "./step-guard";
import type { Command } from "./types";

/** A guard on a clock the test moves, and the commands it let through. */
function guarded() {
  let ms = 1_000;
  const guard = createStepGuard(() => ms);
  const sent: Command["type"][] = [];
  const answers: (() => void)[] = [];
  const send = (command: Command) =>
    guard.send(command, () => {
      sent.push(command.type);
      return new Promise<void>((resolve) => answers.push(resolve));
    });
  return { send, sent, answer: () => answers.shift()?.(), wait: (by: number) => (ms += by), shown: () => guard.shown() };
}

describe("step guard", () => {
  it("drops a second step sent too soon after the first, also once it was answered", async () => {
    const show = guarded();
    expect(show.send({ type: "next" })).not.toBeNull();
    expect(show.send({ type: "next" })).toBeNull();
    show.answer();
    await Promise.resolve();
    show.wait(STEP_PAUSE_MS - 1);
    expect(show.send({ type: "closeQuestion" })).toBeNull();
    show.wait(1);
    expect(show.send({ type: "next" })).not.toBeNull();
    expect(show.sent).toEqual(["next", "next"]);
  });

  it("holds the next step until the last one is answered, however long that takes", async () => {
    const show = guarded();
    const first = show.send({ type: "judgeWrong" });
    show.wait(5 * STEP_PAUSE_MS);
    expect(show.send({ type: "judgeWrong" })).toBeNull();
    show.answer();
    await first;
    show.wait(STEP_PAUSE_MS);
    expect(show.send({ type: "judgeWrong" })).not.toBeNull();
    expect(show.sent).toEqual(["judgeWrong", "judgeWrong"]);
  });

  it("counts the pause from the answer, so a slow answer does not use it up", async () => {
    const show = guarded();
    const first = show.send({ type: "next" });
    show.wait(2 * STEP_PAUSE_MS);
    show.answer();
    await first;
    expect(show.send({ type: "next" })).toBeNull();
    show.wait(STEP_PAUSE_MS);
    expect(show.send({ type: "next" })).not.toBeNull();
    expect(show.sent).toEqual(["next", "next"]);
  });

  it("drops a step sent right after one the show took by itself, meant for the step before", () => {
    const show = guarded();
    show.shown();
    expect(show.send({ type: "next" })).toBeNull();
    show.wait(STEP_PAUSE_MS);
    expect(show.send({ type: "next" })).not.toBeNull();
    expect(show.sent).toEqual(["next"]);
  });

  it("lets presses and score changes through at any rate", () => {
    const show = guarded();
    show.send({ type: "next" });
    for (const buzzer of [1, 2, 3]) expect(show.send({ type: "press", buzzer, button: "big" })).not.toBeNull();
    expect(show.send({ type: "adjustScore", buzzer: 1, delta: 10 })).not.toBeNull();
    expect(show.send({ type: "adjustScore", buzzer: 1, delta: 10 })).not.toBeNull();
    expect(show.sent).toEqual(["next", "press", "press", "press", "adjustScore", "adjustScore"]);
  });

  it("lets a confirmed restart through right after the clock was paused for its question", async () => {
    const show = guarded();
    const paused = show.send({ type: "pauseClock" });
    show.answer();
    await paused;
    expect(show.send({ type: "restartQuestion" })).not.toBeNull();
    expect(show.sent).toEqual(["pauseClock", "restartQuestion"]);
  });

  it("lets the next step through once the last one was refused", async () => {
    let ms = 0;
    const guard = createStepGuard(() => ms);
    await expect(guard.send({ type: "next" }, () => Promise.reject(new Error("notNow")))).rejects.toThrow("notNow");
    ms += STEP_PAUSE_MS;
    expect(guard.send({ type: "next" }, () => Promise.resolve())).not.toBeNull();
  });
});
