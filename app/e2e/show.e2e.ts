// A whole show of the demo quiz in the real app, as on the night: the
// quizmaster's buttons and keys in the Director, presses from the on-screen
// buzzers, the projector checked at every step, and the backend's state
// checked after every step.

import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { Button } from "../src/types";
import { App, t, waitFor } from "./harness";

let app: App;

beforeAll(async () => {
  app = await App.launch();
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

/** Presses `button` on on-screen buzzer `buzzer` (Virtual buzzers tab). */
async function buzz(buzzer: number, button: Button): Promise<void> {
  await app.page.click(await app.$(`.buzzer-grid .buzzer:nth-child(${buzzer}) .key-${button}`));
}

const phaseOf = (view: { phase: { phase: string } & Record<string, unknown> }) =>
  view.phase.phase === "question" ? `question:${String(view.phase.step)}` : view.phase.phase;

async function waitPhase(phase: string, slide?: number) {
  return app.waitView(`phase ${phase}${slide === undefined ? "" : ` on slide ${slide + 1}`}`, (view) => phaseOf(view) === phase && (slide === undefined || view.slideIndex === slide));
}

async function next(phase: string, slide?: number) {
  await app.step("next");
  return waitPhase(phase, slide);
}

/** The projector's text now. */
async function projectorText(): Promise<string> {
  return app.inWindow("host", () => app.bodyText());
}

async function onProjector<T>(work: () => Promise<T>): Promise<T> {
  return app.inWindow("host", work);
}

function score(view: { players: { buzzer: number; score: number }[] }, buzzer: number): number {
  const player = view.players.find((p) => p.buzzer === buzzer);
  if (!player) throw new Error(`buzzer ${buzzer} is not in the show`);
  return player.score;
}

describe("a show of the demo quiz", () => {
  test("starts from the Director and opens the projector", async () => {
    expect(await app.view()).toBeNull();
    await app.seeText(t("noShow"));
    await app.press("demoQuiz");
    const view = await waitPhase("lobby");
    expect(view.quizTitle).toBe("Ukázkový kvíz");
    expect(await app.hasButton("next")).toBe(true);
    await app.press({ text: `${t("projectorToggle")}: ${t("openProjector")}` });
    await onProjector(() => app.seeText("Ukázkový kvíz"));
    await app.seeText(`${t("projectorToggle")}: ${t("closeProjector")}`);
  });

  test("registers buzzers in the lobby, on screen and on the projector", async () => {
    await app.tab("tabBuzzers");
    for (const buzzer of [1, 2, 3, 4]) await buzz(buzzer, "big");
    await app.waitView("four players", (view) => view.players.length === 4);
    await onProjector(() => app.seeText(t("registered", { n: 4 })));
    await app.seeText(t("registered", { n: 4 }));
  });

  test("everyone answers: the projector keeps the answer until the reveal", async () => {
    await next("billboard", 0);
    await onProjector(() => app.seeText("1. kolo: Česko"));
    await next("question:ready", 1);
    await next("question:running", 1);
    await onProjector(async () => {
      await app.seeText("Které město je hlavním městem Moravskoslezského kraje?");
      expect(await app.count(".is-correct")).toBe(0);
      expect(await app.count(".answer-count")).toBe(0);
    });
    // Ostrava (B) is right.
    await buzz(1, "b");
    await buzz(2, "a");
    await buzz(3, "b");
    await app.waitView("three answers", (view) => view.answeredCount === 3);
    await onProjector(async () => expect(await app.count(".is-correct")).toBe(0));
    // The last answer closes the question by itself.
    await buzz(4, "d");
    const closed = await waitPhase("question:closed", 1);
    expect(score(closed, 1)).toBeGreaterThan(50);
    expect(score(closed, 3)).toBeGreaterThan(50);
    expect(score(closed, 2)).toBe(0);
    expect(score(closed, 4)).toBe(0);
    expect(closed.answerCounts).toEqual([1, 2, 0, 1]);
    await next("question:revealed", 1);
    await onProjector(async () => {
      await waitFor("the right answer marked", async () => (await app.count(".answer-1.is-correct")) === 1);
      expect(await app.count(".answer-count")).toBe(4);
    });
  });

  test("a second answer of the same buzzer does not count", async () => {
    await next("question:ready", 2);
    await next("question:running", 2);
    await buzz(2, "c");
    await app.waitView("one answer", (view) => view.answeredCount === 1);
    const before = await app.view();
    await buzz(2, "a");
    await buzz(2, "c");
    const after = await app.view();
    expect(after?.answeredCount).toBe(1);
    expect(after?.players.find((p) => p.buzzer === 2)?.answer).toBe(before?.players.find((p) => p.buzzer === 2)?.answer);
  });

  test("the quizmaster's keys pause and resume the clock, and End question closes it", async () => {
    await app.stepKeys("p");
    await app.waitView("the clock paused", (view) => view.controls.resumeClock);
    const paused = (await app.view())?.phase as { clock?: { elapsedBeforeMs: number } };
    await app.stepKeys("p");
    await app.waitView("the clock running", (view) => view.controls.pauseClock);
    expect(paused.clock?.elapsedBeforeMs).toBeGreaterThan(0);
    await app.step("endQuestion");
    const closed = await waitPhase("question:closed", 2);
    // Buzzer 2 answered C (right, Vltava); nobody else answered and there is no penalty.
    expect(score(closed, 2)).toBeGreaterThan(50);
    await app.stepKeys(" ");
    await waitPhase("question:revealed", 2);
  });

  test("Restart question asks, then clears the answers given", async () => {
    await next("question:ready", 3);
    await next("question:running", 3);
    await buzz(2, "a");
    await app.waitView("one answer", (view) => view.answeredCount === 1);
    await app.answerQuestions(true);
    await app.step("restartQuestion");
    const ready = await waitPhase("question:ready", 3);
    expect(await app.asked()).toContain(t("restartQuestionConfirm"));
    expect(ready.answeredCount).toBe(0);
  });

  test("time up closes the question from the show thread", async () => {
    await next("question:running", 3);
    await buzz(1, "c");
    // 15 s clock, then the press grace; nobody touches the Director meanwhile.
    const closed = await app.waitView("time up", (view) => phaseOf(view) === "question:closed", 25_000);
    expect(closed.controls.closeQuestion).toBe(false);
    expect(closed.answeredCount).toBe(1);
    await next("question:revealed", 3);
  }, 60_000);

  test("explanation, end of round and the leaderboard key", async () => {
    await next("explanation", 4);
    await onProjector(() => app.seeText("Sněžka (1603 m)"));
    await next("endOfRound", 5);
    await app.stepKeys("s");
    await app.waitView("the leaderboard shown", (view) => view.leaderboardVisible);
    await onProjector(() => app.$(".leaderboard"));
    await app.stepKeys("s");
    await app.waitView("the leaderboard hidden", (view) => !view.leaderboardVisible);
  });

  test("fastest finger: a wrong buzz locks out, the next buzz is judged right", async () => {
    await next("billboard", 6);
    await next("question:ready", 7);
    await next("question:running", 7);
    await onProjector(() => app.seeText(t("waitingForBuzz")));
    const before = await app.view();
    await buzz(3, "big");
    await waitPhase("question:buzzed", 7);
    expect(await app.hasButton("right")).toBe(true);
    expect(await app.hasButton("next")).toBe(false);
    await app.step("wrong");
    let running = await waitPhase("question:running", 7);
    expect(score(running, 3)).toBe(score(before!, 3) - 30);
    expect(running.players.find((p) => p.buzzer === 3)?.lockedOut).toBe(true);
    // Restarting takes the penalty back and lets buzzer 3 play again.
    await app.answerQuestions(true);
    await app.step("restartQuestion");
    const ready = await waitPhase("question:ready", 7);
    expect(score(ready, 3)).toBe(score(before!, 3));
    expect(ready.players.find((p) => p.buzzer === 3)?.lockedOut).toBe(false);
    await next("question:running", 7);
    await buzz(3, "big");
    await waitPhase("question:buzzed", 7);
    await app.step("wrong");
    running = await waitPhase("question:running", 7);
    expect(score(running, 3)).toBe(score(before!, 3) - 30);
    // A locked-out buzzer cannot buzz again.
    await buzz(3, "big");
    await buzz(4, "big");
    const buzzed = await waitPhase("question:buzzed", 7);
    expect(buzzed.buzzed).toBe(4);
    await app.stepKeys("y");
    const revealed = await waitPhase("question:revealed", 7);
    expect(score(revealed, 4)).toBe(score(before!, 4) + 150);
  });

  test("keyboard buzzers buzz too", async () => {
    await next("question:ready", 8);
    await next("question:running", 8);
    await app.stepKeys("2");
    const buzzed = await waitPhase("question:buzzed", 8);
    expect(buzzed.buzzed).toBe(2);
    await app.stepKeys("n");
    await waitPhase("question:running", 8);
    await app.step("endQuestion");
    await waitPhase("question:closed", 8);
    await next("question:revealed", 8);
  });

  test("the final ranking reveals step by step and the show ends there", async () => {
    await next("endOfRound", 9);
    let view = await next("final", 10);
    while (view.controls.next) {
      const step = (view.phase as { step: number }).step;
      await app.step("next");
      view = await app.waitView(`final step ${step + 1}`, (v) => phaseOf(v) === "final" && (v.phase as { step: number }).step === step + 1);
    }
    expect(await app.hasButton("next")).toBe(false);
    const ranking = await projectorText();
    for (const player of view.players) expect(ranking).toContain(String(player.score));
  });

  test("nothing failed in either window", async () => {
    await app.assertHealthy();
  });

  test("End show asks, closes the show and offers to resume it", async () => {
    await app.answerQuestions(false);
    await app.press("endShow");
    await waitFor("the question", async () => (await app.asked()).includes(t("endShowConfirm")));
    expect(await app.view()).not.toBeNull();
    await app.answerQuestions(true);
    await app.press("endShow");
    await waitFor("the show to end", async () => (await app.view()) === null);
    await app.seeText(t("resume"));
    await app.assertHealthy();
  });
});
