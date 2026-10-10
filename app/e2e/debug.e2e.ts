import { appendFileSync, writeFileSync } from "node:fs";
import { test } from "vitest";
import { App, sleep } from "./harness";
import { Key } from "./webdriver";
const log = (...a: unknown[]) => appendFileSync("/tmp/dbg.txt", JSON.stringify(a) + "\n");
test("dbg", async () => {
  const app = await App.launch();
  await app.press("quizEditor");
  await app.$(".studio");
  await sleep(1000);
  log(await app.bodyText());
  await app.page.keys(["n"], [Key.Control]);
  await sleep(1000);
  log(await app.bodyText());
  writeFileSync("/tmp/shot.png", Buffer.from(await app.page.screenshot(), "base64"));
  await app.quit(); app.forget();
});
