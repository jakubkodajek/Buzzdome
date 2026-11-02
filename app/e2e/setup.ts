// After a failed test, keeps what each window showed (a screenshot and its
// text) in e2e-output/, which CI uploads; after each file, makes sure no app
// it started is left running, whatever its own clean-up did.

import { mkdirSync, writeFileSync } from "node:fs";
import { afterAll, afterEach } from "vitest";
import { join } from "node:path";
import { App, OUTPUT } from "./harness";

afterEach(async (context) => {
  const app = App.running;
  // An app that ended (a failed launch keeps its own record) has no window to read.
  if (context.task.result?.state !== "fail" || !app?.alive) return;
  mkdirSync(OUTPUT, { recursive: true });
  const name = context.task.name.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 80);
  for (const label of ["director", "host"] as const) {
    try {
      await app.window(label);
      writeFileSync(join(OUTPUT, `${name}.${label}.png`), Buffer.from(await app.page.screenshot(), "base64"));
      writeFileSync(join(OUTPUT, `${name}.${label}.txt`), `${await app.bodyText()}\n\nerrors:\n${(await app.errors()).join("\n")}\n`);
    } catch {
      // That window is not open, or the app is gone: nothing to keep.
    }
  }
  // The file's next tests go on in the Director, as after any other test.
  await app.window("director").catch(() => undefined);
});

afterAll(async () => {
  await App.running?.quit();
  App.running = null;
});
