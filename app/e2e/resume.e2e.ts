// The crash drill in the real app: the app is killed mid-show, as a crash
// or a pulled power cable would stop it, then started again on the same
// data, and the quizmaster resumes the show from the Director.

import { afterAll, describe, expect, test } from "vitest";
import type { Button, GameView } from "../src/types";
import { App, dataFolder, sleep, t } from "./harness";

const data = dataFolder();
let app: App;

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

async function buzz(buzzer: number, button: Button): Promise<void> {
  await app.page.click(await app.$(`.buzzer-grid .buzzer:nth-child(${buzzer}) .key-${button}`));
}

const step = (view: GameView) => (view.phase.phase === "question" ? `question:${view.phase.step}` : view.phase.phase);

async function nextTo(phase: string): Promise<GameView> {
  await app.step("next");
  return app.waitView(phase, (view) => step(view) === phase);
}

/** Kills the app and starts it again on the same data; the Director offers to resume. */
async function crashAndRestart(): Promise<void> {
  await app.crash();
  app = await App.launch(data);
  await app.seeText(t("resume"));
}

async function resume(): Promise<GameView> {
  await app.press("resume");
  return app.waitView("the resumed show", () => true);
}

/** What must survive a crash: who plays, their names and scores. */
const players = (view: GameView) => view.players.map(({ buzzer, name, score, enabled }) => ({ buzzer, name, score, enabled }));

describe("a show killed mid-way resumes", () => {
  test("a question killed while its clock ran starts again, answers taken back", async () => {
    app = await App.launch(data);
    await app.press("demoQuiz");
    await app.tab("tabBuzzers");
    for (const buzzer of [1, 2, 3]) await buzz(buzzer, "big");
    await app.waitView("three players", (view) => view.players.length === 3);
    await nextTo("billboard");
    await nextTo("question:ready");
    await nextTo("question:running");
    await buzz(1, "b");
    await buzz(2, "a");
    const before = await app.waitView("two answers", (view) => view.answeredCount === 2);

    await crashAndRestart();
    const resumed = await resume();
    expect(resumed.slideIndex).toBe(before.slideIndex);
    expect(step(resumed)).toBe("question:ready");
    expect(resumed.answeredCount).toBe(0);
    expect(players(resumed)).toEqual(players(before));

    // The show plays on from there.
    await nextTo("question:running");
    await app.tab("tabBuzzers");
    for (const buzzer of [1, 2, 3]) await buzz(buzzer, "b");
    const closed = await app.waitView("closed", (view) => step(view) === "question:closed");
    expect(closed.players.filter((p) => p.score > 0).map((p) => p.buzzer)).toEqual([1, 2, 3]);
    await nextTo("question:revealed");
  });

  test("a revealed question and its scores are exactly as before", async () => {
    const before = (await app.view())!;
    await crashAndRestart();
    const resumed = await resume();
    expect(step(resumed)).toBe("question:revealed");
    expect(resumed.slideIndex).toBe(before.slideIndex);
    expect(players(resumed)).toEqual(players(before));
  });

  test("a fastest-finger buzz waiting for the verdict is restarted, its penalty not charged twice", async () => {
    // To the first fastest-finger question.
    await app.answerQuestions(true);
    let view = (await app.view())!;
    while (view.slideIndex < 7 || step(view) !== "question:ready") view = await nextAny();
    await nextTo("question:running");
    await app.tab("tabBuzzers");
    await buzz(2, "big");
    await app.waitView("buzzed", (v) => step(v) === "question:buzzed");
    await app.step("wrong");
    const charged = await app.waitView("locked out", (v) => step(v) === "question:running" && v.players.some((p) => p.lockedOut));
    await buzz(3, "big");
    await app.waitView("buzzed again", (v) => step(v) === "question:buzzed" && v.buzzed === 3);

    await crashAndRestart();
    const resumed = await resume();
    expect(step(resumed)).toBe("question:ready");
    expect(resumed.players.some((p) => p.lockedOut)).toBe(false);
    // The wrong answer's penalty was taken back with the question, once.
    const penalised = charged.players.find((p) => p.buzzer === 2)!;
    expect(resumed.players.find((p) => p.buzzer === 2)!.score).toBe(penalised.score + 30);
  });

  test("an ended show can still be resumed after a restart", async () => {
    await app.answerQuestions(true);
    await app.press("endShow");
    await app.seeText(t("resume"));
    await app.quit();
    app = await App.launch(data);
    const resumed = await resume();
    expect(step(resumed)).toBe("question:ready");
    await app.assertHealthy();
  });
});

/** Next from wherever the show is, waiting for the step to change. */
async function nextAny(): Promise<GameView> {
  const before = (await app.view())!;
  await sleep(50);
  await app.step("next");
  return app.waitView("the next step", (view) => view.slideIndex !== before.slideIndex || step(view) !== step(before));
}
