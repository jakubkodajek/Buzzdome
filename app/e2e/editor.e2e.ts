// The quiz editor in the real app: a quiz written with the keys and the
// ribbon, every ribbon button clicked, a test run, the quiz's settings in
// one, saving through the system's file picker, and the draft surviving a
// crash.

import { existsSync, readdirSync, readFileSync, renameSync } from "node:fs";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import type { StringKey } from "../src/i18n";
import { App, dataFolder, sleep, t, waitFor } from "./harness";
import { Key } from "./webdriver";

const data = dataFolder();
let app: App;

beforeAll(async () => {
  app = await App.launch(data);
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

const RIBBON_TABS: StringKey[] = ["ribbonHome", "ribbonInsert", "ribbonDesign", "ribbonQuestion", "ribbonView"];

/**
 * Ribbon buttons the crawl leaves alone: they open the system's file
 * picker (the save test drives one), leave the editor, or start a test run
 * (tested on its own).
 */
const NOT_CLICKED: (StringKey | { text: string })[] = [
  "mediaPicture",
  "mediaVideo",
  "mediaSound",
  "mediaBackground",
  "changeFile",
  "importFromSheet",
  "saveSheetTemplate",
  "exportToSheet",
  "openFile",
  "saveQuiz",
  "saveQuizAs",
  "closeQuiz",
  "backToDirector",
  "testFromStart",
  "testFromHere",
  "ribbonFile",
];

const slideCount = () => app.count(".thumbnails .thumbnail-number");

/** Each editor draft of the app as it is on disk (`quiz.json` in a `draft-*` folder, editor.rs). */
function drafts(): { slides: unknown[]; theme?: string | null }[] {
  const found = [];
  for (const folder of app.storage.filter((f) => existsSync(f))) {
    for (const entry of readdirSync(folder, { recursive: true, encoding: "utf8" })) {
      if (!/(^|[\\/])draft-[^\\/]+[\\/]quiz\.json$/.test(entry)) continue;
      try {
        found.push(JSON.parse(readFileSync(join(folder, entry), "utf8")) as { slides: unknown[]; theme?: string | null });
      } catch {
        // Being written right now.
      }
    }
  }
  return found;
}

/** How many slides each draft holds on disk. */
const draftSlides = () => drafts().map((draft) => draft.slides.length);
/** The theme each draft has on disk. */
const draftThemes = () => drafts().map((draft) => draft.theme ?? null);

async function ctrl(key: string, shift = false): Promise<void> {
  await app.page.keys([key], shift ? [Key.Control, Key.Shift] : [Key.Control]);
}

async function ribbonTab(label: StringKey): Promise<void> {
  const literal = `"${t(label)}"`;
  const [tab] = await app.page.findAllXPath(`//nav[contains(@class, 'ribbon-tabs')]/button[normalize-space(.) = ${literal}]`);
  if (!tab) throw new Error(`no ribbon tab ${t(label)}`);
  await app.page.click(tab);
}

describe("the quiz editor", () => {
  test("opens from the Director and starts a new quiz", async () => {
    await app.press("quizEditor");
    await app.$(".studio");
    // Without a quiz the editor opens on its File page.
    await app.$(".backstage");
    await ctrl("n");
    await app.$(".ribbon");
    await app.seeText(t("noSlides").slice(0, 20));
  });

  test("adds questions with the keys, and undo and redo take them back and forth", async () => {
    const start = await slideCount();
    for (let i = 0; i < 3; i++) {
      await ctrl("m");
      await waitFor(`${start + i + 1} slides`, async () => (await slideCount()) === start + i + 1);
    }
    await ctrl("z");
    await waitFor("one undone", async () => (await slideCount()) === start + 2);
    await ctrl("y");
    await waitFor("one redone", async () => (await slideCount()) === start + 3);
  });

  test("every ribbon button on every ribbon tab works", async () => {
    for (const tab of RIBBON_TABS) {
      await ribbonTab(tab);
      await app.clickEverything(".ribbon-body", NOT_CLICKED);
      // A gallery or menu a button opened closes again.
      await app.page.keys([Key.Escape]);
    }
    await app.assertHealthy();
    // Back to the slides (the crawl also chose the table and category views), with a question to play.
    await ribbonTab("ribbonView");
    await app.press("viewGraphical");
    await ctrl("m");
    await waitFor("a slide", async () => (await slideCount()) > 0);
  });

  test("a test run plays the edited quiz and goes back to the editor", async () => {
    await ribbonTab("ribbonHome");
    const slides = await slideCount();
    await app.page.keys([Key.F5]);
    await app.seeText(t("testRunActive"));
    const view = await app.waitView("the test show", (v) => v.slideCount === slides);
    expect(view.phase.phase).toBe("lobby");
    await app.step("next");
    await app.waitView("the first slide", (v) => v.phase.phase !== "lobby");
    await app.press("backToEditor");
    await app.$(".studio");
    // The thumbnails draw a moment after the editor opens: wait, never read once.
    await waitFor(`${slides} slides back`, async () => (await slideCount()) === slides);
    expect(await app.view()).toBeNull();
  });

  test("the quiz's settings show the right answer by itself once everyone answered", async () => {
    await app.press("ribbonFile");
    await app.press("quizSettings");
    await app.page.click(await app.$(`select[aria-label="${t("allAnswered")}"] option[value="reveal"]`));
    const delay = await app.$(`input[aria-label="${t("allAnsweredDelay")}"]`);
    await app.page.clear(delay);
    await app.page.type(delay, "1");
    await app.page.click(await app.$(".backstage-back"));
    // A test run from the question added last (`ctrl+m` above), which is selected.
    await app.page.keys([Key.F5], [Key.Shift]);
    await app.seeText(t("testRunActive"));
    await app.waitView("the question", (v) => v.phase.phase === "question" && v.phase.step === "ready");
    // A press registers a buzzer whenever it comes.
    await app.tab("tabBuzzers");
    for (const buzzer of [1, 2]) await app.page.click(await app.$(`.buzzer-grid .buzzer:nth-child(${buzzer}) .key-big`));
    await app.waitView("two players", (v) => v.players.length === 2);
    await app.step("next");
    await app.waitView("the clock running", (v) => v.phase.phase === "question" && v.phase.step === "running");
    for (const buzzer of [1, 2]) await app.page.click(await app.$(`.buzzer-grid .buzzer:nth-child(${buzzer}) .key-a`));
    const closed = await app.waitView("the question closed", (v) => v.phase.phase === "question" && v.phase.step === "closed");
    expect(closed.autoStep?.action).toBe("reveal");
    await app.seeText(t("autoStepReveal"));
    await app.waitView("the answer shown by itself", (v) => v.phase.phase === "question" && v.phase.step === "revealed");
    await app.press("backToEditor");
    await app.$(".studio");
  });

  test("a test run from the start opens on the quiz's welcome screen, and Next goes on to the registration", async () => {
    await app.press("ribbonFile");
    await app.press("quizSettings");
    await app.page.click(await app.$(`button[role="switch"][aria-label="${t("welcomeOn")}"]`));
    await app.page.type(await app.$(".welcome-fields label:first-of-type input"), "Friday quiz");
    await app.page.type(await app.$(".welcome-fields label:last-of-type input"), "Teams of four");
    await app.page.click(await app.$(".backstage-back"));
    await app.page.keys([Key.F5]);
    await app.seeText(t("testRunActive"));
    const welcome = await app.waitView("the welcome screen", (v) => v.phase.phase === "welcome");
    expect(welcome.welcome).toMatchObject({ on: true, heading: "Friday quiz", footer: "Teams of four" });
    // A press registers a buzzer on the welcome screen already.
    await app.tab("tabBuzzers");
    await app.page.click(await app.$(".buzzer-grid .buzzer:nth-child(1) .key-big"));
    await app.waitView("a player", (v) => v.players.length === 1);
    await app.step("next");
    const lobby = await app.waitView("the registration", (v) => v.phase.phase === "lobby");
    expect(lobby.welcome).toBeNull();
    await app.step("next");
    await app.waitView("the first slide", (v) => v.slide !== null);
    await app.press("backToEditor");
    await app.$(".studio");
  });

  test("the draft survives a crash", async () => {
    // Back from a test run, the editor shows the slides once it has read its draft.
    const slides = await waitFor("the slides", async () => (await slideCount()) || null);
    // The draft is written a moment after each change: wait until it is on disk.
    await waitFor("the draft on disk", async () => draftSlides().includes(slides));
    await app.crash();
    app = await App.launch(data);
    await app.press("quizEditor");
    await app.$(".studio");
    await waitFor(`${slides} slides back`, async () => (await slideCount()) === slides);
  });

  // The system's file picker is driven through the X server (`App.pickFile`).
  test.skipIf(process.platform !== "linux")("Save as writes a quiz file through the system's picker, and the Director plays it, again from its recent quizzes", async () => {
    const file = join(data, "saved-quiz.quiz");
    await ctrl("s", true);
    await app.pickFile(file);
    await waitFor("the quiz file", async () => existsSync(file));
    // A quiz file is a ZIP (docs/quiz-format.md).
    expect(readFileSync(file).subarray(0, 2).toString("latin1")).toBe("PK");
    const slides = await slideCount();
    await app.press("backToDirector");
    await app.press("openQuiz");
    await app.pickFile(file);
    const view = await app.waitView("the saved quiz's show", () => true);
    expect(view.slideCount).toBe(slides);
    await app.answerQuestions(true);
    await app.press("endShow");
    await waitFor("the show to end", async () => (await app.view()) === null);
    // Opened as files often are, with a double click: it starts one show.
    await app.page.doubleClick(await app.button({ text: "saved-quiz.quiz" }));
    const again = await app.waitView("the show of the recent quiz", () => true);
    expect(again.slideCount).toBe(slides);
    // A second start would end in a message once the first show began.
    await sleep(1_000);
    expect(await app.count(".toast")).toBe(0);
    await app.assertHealthy();
  });

  test.skipIf(process.platform !== "linux")("a recent quiz whose file is gone leaves the list, and no show starts", async () => {
    const file = join(data, "saved-quiz.quiz");
    await app.answerQuestions(true);
    await app.press("endShow");
    await waitFor("the show to end", async () => (await app.view()) === null);
    await app.tab("tabQuestions");
    await app.seeText(t("recentFiles"));
    renameSync(file, `${file}.moved`);
    try {
      await app.press({ text: "saved-quiz.quiz" });
      await waitFor("the file gone from the list", async () => !(await app.bodyText()).includes("saved-quiz.quiz"));
      expect(await app.view()).toBeNull();
      expect(await app.count(".render-failed")).toBe(0);
    } finally {
      renameSync(`${file}.moved`, file);
    }
  });

  test("a theme picked on the Design tab is the quiz's look, in the draft and in a test run", async () => {
    if ((await app.view()) !== null) {
      await app.answerQuestions(true);
      await app.press("endShow");
      await waitFor("the show to end", async () => (await app.view()) === null);
    }
    // Where the tests before it run (Linux), they end in the Director; elsewhere the crash test ends in the editor.
    if ((await app.count(".studio")) === 0) await app.press("quizEditor");
    await app.$(".studio");
    const themes = await app.invoke<{ id: string; name: { cs: string } }[]>("list_themes");
    const current = draftThemes();
    const theme = themes.find((candidate) => !current.includes(candidate.id))!;
    await ribbonTab("ribbonDesign");
    await app.page.click(await app.$(`button[title="${theme.name.cs}"]`));
    await waitFor(`the draft in ${theme.id}`, async () => draftThemes().includes(theme.id));
    await app.page.keys([Key.F5], [Key.Shift]);
    await app.seeText(t("testRunActive"));
    expect((await app.invoke<{ theme: string | null } | null>("get_quiz"))?.theme).toBe(theme.id);
    await app.assertHealthy();
    await app.press("backToEditor");
    await app.$(".studio");
  });
});
