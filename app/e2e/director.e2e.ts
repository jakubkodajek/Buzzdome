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

// The Lights tab has its own crawl (lights.e2e.ts): with a button or more for
// each of its looks, a crawl of it takes long enough to need a test of its own.
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

describe("the Director", () => {
  test("every tab and button works before a show", async () => {
    for (const tab of TABS) {
      await app.tab(tab);
      await app.clickEverything(".tab-body", NOT_CLICKED);
    }
    await app.assertHealthy();
    expect(await app.view()).toBeNull();
  });

  test("every tab and button works during a show", async () => {
    await app.press("demoQuiz");
    await app.waitView("the show", () => true);
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
    await app.assertHealthy();
  });

  test("Questions: jumping to a slide and to the final", async () => {
    await app.tab("tabQuestions");
    await app.page.click(await app.$(".slide-grid tbody tr:nth-child(3) td:nth-child(1)"));
    await app.step("goToSelected");
    await app.waitView("slide 3", (v) => v.slideIndex === 2);
    await app.answerQuestions(true);
    // A jump is a step too: the step guard drops one that follows another too fast.
    await app.step("goToFinal");
    expect(await app.asked()).toContain(t("goToFinalConfirm"));
    await app.waitView("the final", (v) => v.phase.phase === "final");
  });

  test("Settings: the language switches the whole Director and back", async () => {
    await app.tab("tabSettings");
    const select = async () => {
      for (const element of await app.page.findAll(".tab-body select")) {
        const options = await app.page.execute<string[]>("return [...arguments[0].options].map((o) => o.value);", [element]);
        if (options.includes("en") && options.includes("cs")) return element;
      }
      throw new Error("no language list");
    };
    const choose = async (lang: string) =>
      app.page.execute("const s = arguments[0]; s.value = arguments[1]; s.dispatchEvent(new Event('change', { bubbles: true })); return true;", [await select(), lang]);
    await choose("en");
    const en = translator("en");
    await app.seeText(en("tabSettings"));
    await waitFor("English buttons", () => app.hasButton("next").then((czech) => !czech || t("next") === en("next")));
    await app.assertHealthy();
    await choose("cs");
    await app.seeText(t("tabSettings"));
  });

  test("nothing failed in either window", async () => {
    await app.assertHealthy();
  });
});
