// Every Director tab in the real app, before a show and during one: each
// button clicked once (nothing may break), then the controls that change a
// show checked for what they do.

import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { StringKey } from "../src/i18n";
import { translator } from "../src/i18n";
import type { GameView } from "../src/types";
import { App, t, waitFor } from "./harness";

let app: App;

beforeAll(async () => {
  app = await App.launch();
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

const TABS: StringKey[] = ["tabQuestions", "tabTeams", "tabResponses", "tabBuzzers", "tabMedia", "tabReceiver", "tabSettings"];

/**
 * Buttons the crawl leaves alone: they open the system's file picker, which
 * the file tests drive, leave the Director (the editor has its own tests),
 * end the show or the app, or start one.
 */
const NOT_CLICKED: (StringKey | { text: string })[] = [
  "openQuiz",
  "importTeams",
  "exportLog",
  "mediaAddHint",
  "quizEditor",
  "endShow",
  "closeAppKeepShow",
  "demoQuiz",
  "resume",
];

const player = (view: GameView, buzzer: number) => {
  const found = view.players.find((p) => p.buzzer === buzzer);
  if (!found) throw new Error(`no buzzer ${buzzer}`);
  return found;
};

/** Chooses `value` in the Settings tab's list that offers all of `values`. */
async function chooseSetting(value: string, ...values: string[]): Promise<void> {
  for (const element of await app.page.findAll(".tab-body select")) {
    const options = await app.page.execute<string[]>("return [...arguments[0].options].map((o) => o.value);", [element]);
    if (values.every((v) => options.includes(v))) {
      await app.page.execute("const s = arguments[0]; s.value = arguments[1]; s.dispatchEvent(new Event('change', { bubbles: true })); return true;", [element, value]);
      return;
    }
  }
  throw new Error(`no list of ${values.join(", ")}`);
}

describe("the Director", () => {
  test("every tab and button works before a show", async () => {
    for (const tab of TABS) {
      await app.tab(tab);
      await app.clickEverything(".tab-body", NOT_CLICKED);
    }
    await app.assertHealthy();
    expect(await app.view()).toBeNull();
  });

  test("the check before the show: what is not ready, a fix for each, then hidden", async () => {
    await app.press("demoQuiz");
    await app.waitView("the lobby", (v) => v.phase.phase === "lobby");
    await app.seeText(t("launchCheck"));
    // The receiver, the buzzers, the projector, the sound and the disk, whose free space the system tells.
    expect(await app.count(".launch-item")).toBe(5);
    await app.seeText(t("launchNoBuzzers"));
    await app.tab("tabBuzzers");
    await app.page.click(await app.$(".buzzer-grid .buzzer:nth-child(1) .key-big"));
    await app.seeText(t("launchBuzzersRegistered", { players: 1 }));
    await app.seeText(t("launchProjectorClosed"));
    await app.press("launchOpenProjector");
    await waitFor("the projector open", async () => !(await app.bodyText()).includes(t("launchProjectorClosed")));
    await app.press("launchPlaySound");
    await app.press("launchSoundHeardButton");
    await app.seeText(t("launchSoundHeard"));
    await app.assertHealthy();
    await app.press("launchHide");
    await waitFor("the check hidden", async () => (await app.count(".launch-check")) === 0);
  });

  test("every tab and button works during a show", async () => {
    // The show the check before it started, still in the lobby.
    await app.tab("tabBuzzers");
    for (const buzzer of [1, 2, 3]) await app.page.click(await app.$(`.buzzer-grid .buzzer:nth-child(${buzzer}) .key-big`));
    for (const tab of TABS) {
      await app.tab(tab);
      await app.clickEverything(".tab-body", NOT_CLICKED);
    }
    await app.assertHealthy();
    expect(await app.view()).not.toBeNull();
  });

  test("the command panel's buttons all work, step after step, to the end of the show", async () => {
    // Whatever the step offers, clicked as it comes: a show played with the panel alone.
    await app.answerQuestions(true);
    for (let round = 0; round < 80; round++) {
      const view = (await app.view())!;
      if (!view.controls.next && !view.controls.judge) break;
      await app.clickEverything(".command-panel", [...NOT_CLICKED, "restartQuestion", "projectorToggle"]);
      if (view.controls.judge) await app.step("right");
      else await app.step("next");
      await app.waitView("the next step", (v) => v.slideIndex !== view.slideIndex || JSON.stringify(v.phase) !== JSON.stringify(view.phase));
    }
    const end = (await app.view())!;
    expect(end.controls.next).toBe(false);
    await app.assertHealthy();
  });

  test("Teams: scores, disabling and names change the show", async () => {
    await app.tab("tabTeams");
    const row = (buzzer: number) => app.$(`.teams-tab tbody tr:nth-child(${buzzer}) td:nth-child(3)`);
    const before = (await app.view())!;
    await app.page.click(await row(1));
    await app.press({ text: "+10" });
    await app.waitView("10 more", (v) => player(v, 1).score === player(before, 1).score + 10);
    await app.press({ text: "-1" });
    await app.waitView("1 less", (v) => player(v, 1).score === player(before, 1).score + 9);
    await app.press("disable");
    await app.waitView("disabled", (v) => !player(v, 1).enabled);
    await app.press("enable");
    await app.waitView("enabled", (v) => player(v, 1).enabled);
    // A name typed into the table, with Czech letters.
    const name = await app.$(".teams-tab tbody tr:nth-child(2) .cell-input");
    await app.page.clear(name);
    await app.page.type(name, "Žluťoučký kůň");
    await app.waitView("the name", (v) => player(v, 2).name === "Žluťoučký kůň");
    // A new buzzer by number.
    const number = await app.$(".teams-tab input[type=number]");
    await app.page.type(number, "42");
    await app.press("addPlayer");
    await app.waitView("buzzer 42", (v) => v.players.some((p) => p.buzzer === 42));
    // Every score and time back to zero, once asked; the show stays where it is.
    const slide = (await app.view())!.slideIndex;
    await app.press("resetScores");
    expect(await app.asked()).toContain(t("resetScoresConfirm"));
    await app.waitView("scores reset", (v) => v.slideIndex === slide && v.players.every((p) => p.score === 0 && p.totalAnswerMs === 0));
    await app.assertHealthy();
  });

  test("Questions: jumping to a slide, to the final and back to registration", async () => {
    await app.tab("tabQuestions");
    await app.page.click(await app.$(".slide-grid tbody tr:nth-child(3) td:nth-child(1)"));
    await app.step("goToSelected");
    await app.waitView("slide 3", (v) => v.slideIndex === 2);
    await app.answerQuestions(true);
    // A jump is a step too: the step guard drops one that follows another too fast.
    await app.step("goToFinal");
    expect(await app.asked()).toContain(t("goToFinalConfirm"));
    await app.waitView("the final", (v) => v.phase.phase === "final");
    // Starting over keeps the players and goes back to registration.
    const players = (await app.view())!.players.map((p) => p.buzzer);
    await app.step("startOver");
    expect(await app.asked()).toContain(t("startOverConfirm"));
    const same = (v: GameView) => v.players.map((p) => p.buzzer).join() === players.join();
    await app.waitView("registration again", (v) => v.phase.phase === "lobby" && same(v) && v.players.every((p) => p.score === 0));
  });

  test("Settings: the language switches the whole Director and back", async () => {
    await app.tab("tabSettings");
    const choose = (lang: string) => chooseSetting(lang, "cs", "en");
    await choose("en");
    const en = translator("en");
    await app.seeText(en("tabSettings"));
    await waitFor("English buttons", () => app.hasButton("next").then((czech) => !czech || t("next") === en("next")));
    await app.assertHealthy();
    await choose("cs");
    await app.seeText(t("tabSettings"));
  });

  test("Settings: the dark look darkens the Director and back", async () => {
    await app.tab("tabSettings");
    const choose = (look: string) => chooseSetting(look, "light", "dark", "system");
    const dark = () => app.count(".director.dark");
    expect(await dark()).toBe(0);
    await choose("dark");
    await waitFor("the dark look", async () => (await dark()) === 1);
    await app.assertHealthy();
    await choose("light");
    await waitFor("the light look", async () => (await dark()) === 0);
  });

  test("nothing failed in either window", async () => {
    await app.assertHealthy();
  });
});
