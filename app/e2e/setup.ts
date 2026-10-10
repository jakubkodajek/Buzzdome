// After a failed test, keeps what each window showed (a screenshot and its
// text) in e2e-output/, which CI uploads.

import { mkdirSync, writeFileSync } from "node:fs";
import { afterEach } from "vitest";
import { App } from "./harness";

afterEach(async (context) => {
  const app = App.running;
  if (context.task.result?.state !== "fail" || !app) return;
  const folder = new URL("../e2e-output/", import.meta.url);
  mkdirSync(folder, { recursive: true });
  const name = context.task.name.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 80);
  for (const label of ["director", "host"] as const) {
    try {
      await app.window(label);
      writeFileSync(new URL(`${name}.${label}.png`, folder), Buffer.from(await app.page.screenshot(), "base64"));
      writeFileSync(new URL(`${name}.${label}.txt`, folder), `${await app.bodyText()}\n\nerrors:\n${(await app.errors()).join("\n")}\n`);
    } catch {
      // That window is not open, or the app is gone: nothing to keep.
    }
  }
});
