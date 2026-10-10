// Starts the real Buzzdome app under tauri-driver and drives its windows
// as the quizmaster would. Each `App` gets its own data folder (sessions,
// settings, drafts), so tests never share a show, and a test can kill the
// app and start it again on the same folder to check a resume.
//
// Needs `tauri-driver` on PATH with the platform's WebDriver (WebKitWebDriver
// on PATH on Linux; msedgedriver named by NATIVE_DRIVER on Windows), and an
// app built with `npm run tauri build -- --debug --no-bundle` (or
// BUZZDOME_APP naming one). On Linux the tests run under a virtual display
// (`xvfb-run -a npm run e2e`) and drive the system's file picker with xdotool.

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { translator, type StringKey } from "../src/i18n";
import { STEP_PAUSE_MS } from "../src/step-guard";
import type { GameView } from "../src/types";
import { Session, WebDriverError, type ElementRef } from "./webdriver";

/** The app's own words for a string key, in the UI's default language. */
export const t = translator("cs");

const APP = process.env.BUZZDOME_APP ?? fileURLToPath(new URL(`../../target/debug/buzzdome${process.platform === "win32" ? ".exe" : ""}`, import.meta.url));

/** How long a step may take to show before a test fails. */
const STEP_TIMEOUT_MS = 10_000;

/** The labels of the windows, as `app_windows.rs` names them. */
export type WindowLabel = "director" | "host";

/** A free TCP port on this machine. */
function freePort(): Promise<number> {
  return new Promise((done, fail) => {
    const server = createServer();
    server.unref();
    server.on("error", fail);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => (address && typeof address === "object" ? done(address.port) : fail(new Error("no port"))));
    });
  });
}

export function sleep(ms: number): Promise<void> {
  return new Promise((done) => setTimeout(done, ms));
}

/** Waits until `check` gives a value other than false, null or undefined, and gives it. */
export async function waitFor<T>(what: string, check: () => Promise<T | false | null | undefined>, timeoutMs = STEP_TIMEOUT_MS): Promise<T> {
  const until = Date.now() + timeoutMs;
  let last: unknown = null;
  for (;;) {
    try {
      const value = await check();
      if (value !== false && value !== null && value !== undefined) return value;
    } catch (error) {
      last = error;
    }
    if (Date.now() > until) throw new Error(`timed out waiting for ${what}${last ? `: ${String(last)}` : ""}`);
    await sleep(100);
  }
}

/** A folder for one app's data, kept across a restart of that app. */
export function dataFolder(): string {
  return mkdtempSync(join(tmpdir(), "buzzdome-e2e-"));
}

/** Scripts the tests put into a page: catching errors, and answering the app's yes/no questions. */
const PAGE_SETUP = `
  if (!window.__e2e) {
    window.__e2e = { errors: [], answer: true, asked: [] };
    window.addEventListener("error", (event) => window.__e2e.errors.push(String(event.message)));
    window.addEventListener("unhandledrejection", (event) => window.__e2e.errors.push("unhandled: " + String(event.reason && (event.reason.message || JSON.stringify(event.reason)))));
    const consoleError = console.error.bind(console);
    console.error = (...args) => { window.__e2e.errors.push(args.map((a) => a && a.message ? a.message : typeof a === "string" ? a : JSON.stringify(a)).join(" ")); consoleError(...args); };
    // The app asks yes/no through the dialog plugin (confirm.ts): a system
    // dialog WebDriver cannot reach. The page's IPC is locked, but it sends
    // each call with fetch, so the tests answer that call here instead, and
    // keep what was asked.
    const fetchFromPage = window.fetch.bind(window);
    window.fetch = (input, init) => {
      const url = String(input && input.url ? input.url : input);
      if (/plugin%3Adialog%7Cmessage$/.test(url)) {
        const { message, buttons } = JSON.parse(init.body);
        window.__e2e.asked.push(message);
        // The plugin's ask() sends its Yes and No labels and reads back the one chosen.
        const [yes, no] = (buttons && buttons.OkCancelCustom) || ["Yes", "No"];
        const chosen = window.__e2e.answer ? yes : no;
        return Promise.resolve(new Response(JSON.stringify(chosen), { headers: { "Tauri-Response": "ok", "Content-Type": "application/json" } }));
      }
      return fetchFromPage(input, init);
    };
  }
  return true;
`;

export class App {
  /** The app a test drives now, for keeping what it showed when the test fails (setup.ts). */
  static running: App | null = null;
  private session: Session | null = null;
  private constructor(
    private readonly driver: ChildProcess,
    private readonly base: string,
    readonly data: string,
  ) {}

  /** Starts the app on `data` (a fresh folder unless given) and waits for the Director. */
  static async launch(data = dataFolder()): Promise<App> {
    const [port, nativePort] = [await freePort(), await freePort()];
    // The app keeps its data where the OS says (design.md, section 3); point every such folder at `data`.
    const env = {
      ...process.env,
      HOME: data,
      XDG_DATA_HOME: join(data, "share"),
      XDG_CONFIG_HOME: join(data, "config"),
      XDG_CACHE_HOME: join(data, "cache"),
      APPDATA: join(data, "roaming"),
      LOCALAPPDATA: join(data, "local"),
    };
    // On Windows the platform's driver (msedgedriver) is named by NATIVE_DRIVER; on Linux WebKitWebDriver is on PATH.
    const native = process.env.NATIVE_DRIVER ? ["--native-driver", process.env.NATIVE_DRIVER] : [];
    const driver = spawn("tauri-driver", ["--port", String(port), "--native-port", String(nativePort), ...native], { env, stdio: ["ignore", "inherit", "inherit"] });
    const app = new App(driver, `http://127.0.0.1:${port}`, data);
    await waitFor("tauri-driver", () => fetch(`${app.base}/status`).then((r) => r.ok), 20_000);
    app.session = await Session.start(app.base, { browserName: "wry", "tauri:options": { application: APP } });
    await app.inWindow("director", () => app.$("button"));
    App.running = app;
    return app;
  }

  get page(): Session {
    if (!this.session) throw new Error("the app is not running");
    return this.session;
  }

  /** Switches to the window labelled `label` (the Director or the projector), set up for the tests. */
  async window(label: WindowLabel): Promise<void> {
    const found = await waitFor(`the ${label} window`, async () => {
      for (const handle of await this.page.windowHandles()) {
        await this.page.switchToWindow(handle);
        const current = await this.page.execute<string | null>("return window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? null;");
        if (current === label) return handle;
      }
      return null;
    });
    await this.page.switchToWindow(found);
    await this.page.execute(PAGE_SETUP);
  }

  /** Runs `work` in the window `label`, then goes back to the Director. */
  async inWindow<T>(label: WindowLabel, work: () => Promise<T>): Promise<T> {
    await this.window(label);
    try {
      return await work();
    } finally {
      if (label !== "director") await this.window("director");
    }
  }

  /** The first element `css` matches that is shown, waiting for one. */
  $(css: string): Promise<ElementRef> {
    return waitFor(`an element ${css}`, async () => {
      for (const element of await this.page.findAll(css)) {
        if (await this.page.displayed(element)) return element;
      }
      return null;
    });
  }

  /** How many shown elements `css` matches now. */
  async count(css: string): Promise<number> {
    let shown = 0;
    for (const element of await this.page.findAll(css)) if (await this.page.displayed(element)) shown++;
    return shown;
  }

  /** The text of the page's body, as shown. */
  async bodyText(): Promise<string> {
    return this.page.text(await this.$("body"));
  }

  /** Waits until the page shows `text`. */
  async seeText(text: string, timeoutMs = STEP_TIMEOUT_MS): Promise<void> {
    await waitFor(`the text "${text}"`, async () => (await this.bodyText()).includes(text), timeoutMs);
  }

  /** The shown button whose text contains `label` (a string key's text, or text as is). */
  async button(label: StringKey | { text: string }): Promise<ElementRef> {
    const text = typeof label === "string" ? t(label) : label.text;
    const literal = text.includes('"') ? `'${text}'` : `"${text}"`;
    // A button labelled exactly so comes first: "-1" is not "-10".
    const xpaths = [`//button[normalize-space(.) = ${literal}]`, `//button[contains(normalize-space(.), ${literal})]`];
    return waitFor(`a button "${text}"`, async () => {
      for (const xpath of xpaths) {
        for (const element of await this.page.findAllXPath(xpath)) {
          if ((await this.page.displayed(element)) && (await this.page.enabled(element))) return element;
        }
      }
      return null;
    });
  }

  async hasButton(label: StringKey): Promise<boolean> {
    const literal = `"${t(label)}"`;
    for (const element of await this.page.findAllXPath(`//button[contains(normalize-space(.), ${literal})]`)) {
      if (await this.page.displayed(element)) return true;
    }
    return false;
  }

  async press(label: StringKey | { text: string }): Promise<void> {
    await this.page.click(await this.button(label));
  }

  /**
   * Presses a step button (Next, Right, ...) as a person would: not within
   * `STEP_PAUSE_MS` of the last step, which the Director drops as a double
   * press (step-guard.ts).
   */
  async step(label: StringKey | { text: string }): Promise<void> {
    await sleep(STEP_PAUSE_MS + 100);
    await this.press(label);
  }

  /** Presses the quizmaster's `keys` in the Director, after the step pause as `step` does. */
  async stepKeys(...keys: string[]): Promise<void> {
    await sleep(STEP_PAUSE_MS + 100);
    await this.page.keys(keys);
  }

  /** Clicks the Director tab named by `label`. */
  async tab(label: StringKey): Promise<void> {
    await this.page.click(await this.$(`[role=tab]:nth-child(${await this.tabIndex(label)})`));
  }

  private async tabIndex(label: StringKey): Promise<number> {
    const tabs = await this.page.findAll("[role=tab]");
    for (let i = 0; i < tabs.length; i++) if ((await this.page.text(tabs[i])).trim() === t(label)) return i + 1;
    throw new Error(`no tab ${t(label)}`);
  }

  /** Calls a backend command as the page would, and gives its answer. */
  invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    return this.page.executeAsync<{ ok: T } | { error: unknown }>(
      `const done = arguments[arguments.length - 1];
       window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]).then((ok) => done({ ok }), (error) => done({ error }));`,
      [command, args],
    ).then((answer) => {
      if ("error" in answer) throw new Error(`${command}: ${JSON.stringify(answer.error)}`);
      return answer.ok;
    });
  }

  /** The show as the backend has it now (`get_view`). */
  view(): Promise<GameView | null> {
    return this.invoke<GameView | null>("get_view");
  }

  /** Waits until the show's view passes `check`. */
  async waitView(what: string, check: (view: GameView) => boolean, timeoutMs = STEP_TIMEOUT_MS): Promise<GameView> {
    return waitFor(what, async () => {
      const view = await this.view();
      return view && check(view) ? view : null;
    }, timeoutMs);
  }

  /** How the tests answer the app's next yes/no questions. */
  async answerQuestions(yes: boolean): Promise<void> {
    await this.page.execute("window.__e2e.answer = arguments[0]; return true;", [yes]);
  }

  /** The questions the app asked in this window so far. */
  asked(): Promise<string[]> {
    return this.page.execute("return window.__e2e.asked;");
  }

  /** Errors the current window logged or threw since the tests set it up. */
  errors(): Promise<string[]> {
    return this.page.execute("return window.__e2e ? window.__e2e.errors : [];");
  }

  /** Fails if a window logged an error, or a part of it failed to render. */
  async assertHealthy(): Promise<void> {
    for (const label of ["director", "host"] as const) {
      const handles = await this.page.windowHandles();
      if (label === "host" && handles.length < 2) continue;
      await this.inWindow(label, async () => {
        const errors = await this.errors();
        if (errors.length > 0) throw new Error(`the ${label} window logged errors:\n${errors.join("\n")}`);
        const failed = await this.count(".render-failed, .host-failed");
        if (failed > 0) throw new Error(`a part of the ${label} window failed to render:\n${await this.bodyText()}`);
      });
    }
  }

  /**
   * Clicks every shown, enabled button under `scope` once, as a curious
   * quizmaster would, except those whose text or title holds the words of
   * one of `skip`. After each click the Director must still
   * render, log no error and the backend must still answer. Gives what was
   * clicked, by label.
   */
  async clickEverything(scope: string, skip: (StringKey | { text: string })[] = []): Promise<string[]> {
    const skipped = skip.map((label) => (typeof label === "string" ? t(label) : label.text));
    const clicked = new Set<string>();
    for (let round = 0; round < 300; round++) {
      let target: { element: ElementRef; label: string } | null = null;
      const buttons = await this.page.findAll(`${scope} button`);
      for (let i = 0; i < buttons.length && !target; i++) {
        const element = buttons[i];
        if (!(await this.page.displayed(element)) || !(await this.page.enabled(element))) continue;
        const text = (await this.page.text(element)).trim();
        const title = (await this.page.attribute(element, "title")) ?? (await this.page.attribute(element, "aria-label")) ?? "";
        const label = `${text || title || "(no label)"} #${i}`;
        if (clicked.has(label) || skipped.some((s) => text.includes(s) || title.includes(s))) continue;
        target = { element, label };
      }
      if (!target) return [...clicked];
      clicked.add(target.label);
      try {
        await this.page.click(target.element);
      } catch (error) {
        // The page changed under the click (the button went away): try the next one.
        if (!(error instanceof WebDriverError)) throw error;
        continue;
      }
      await sleep(150);
      const errors = await this.errors();
      if (errors.length > 0) throw new Error(`clicking "${target.label}" logged errors:\n${errors.join("\n")}`);
      if ((await this.count(".render-failed")) > 0) throw new Error(`clicking "${target.label}" broke a part of the window:\n${await this.bodyText()}`);
      await waitFor(`the backend after clicking "${target.label}"`, async () => (await this.view(), true), 5_000);
    }
    throw new Error(`more than 300 buttons under ${scope}`);
  }

  /**
   * Answers the system file picker the app opened with `path`, as a person
   * typing it would: the picker is the desktop's own window, so this goes
   * through the X server (`xdotool`), on Linux only.
   */
  async pickFile(path: string): Promise<void> {
    if (process.platform !== "linux") throw new Error("the file picker is driven on Linux only");
    const xdotool = (...args: string[]) => spawnSync("xdotool", args, { encoding: "utf8" });
    const picker = await waitFor("the file picker", async () => {
      const found = xdotool("search", "--onlyvisible", "--name", "^(Open|Save|Otevřít|Uložit)( File| soubor)?$");
      return found.stdout.trim().split("\n").filter(Boolean).pop() ?? null;
    });
    xdotool("windowfocus", "--sync", picker);
    // Ctrl+L opens the location field, which takes a whole path.
    xdotool("key", "--clearmodifiers", "ctrl+l");
    await sleep(300);
    xdotool("key", "--clearmodifiers", "ctrl+a");
    xdotool("type", "--delay", "5", path);
    await sleep(300);
    xdotool("key", "Return");
    await waitFor("the file picker to close", async () => xdotool("search", "--onlyvisible", "--name", "^(Open|Save|Otevřít|Uložit)( File| soubor)?$").stdout.trim() === "");
  }

  /** Ends the app as the quizmaster would. */
  async quit(): Promise<void> {
    try {
      await this.session?.end();
    } catch (error) {
      if (!(error instanceof WebDriverError)) throw error;
    }
    this.session = null;
    await this.stopDriver();
  }

  /** Kills the app at once, as a crash or a power cut would; its data stays. */
  async crash(): Promise<void> {
    killApp();
    this.session = null;
    await this.stopDriver();
  }

  private async stopDriver(): Promise<void> {
    if (this.driver.exitCode === null) {
      const exited = new Promise((done) => this.driver.once("exit", done));
      this.driver.kill();
      await exited;
    }
    // The app takes a moment to close after its session ends, and writes its
    // caches meanwhile; the next launch or `forget` must not race it.
    const until = Date.now() + 15_000;
    while (appRunning() && Date.now() < until) await sleep(200);
    if (appRunning()) killApp();
  }

  /** Removes the app's data folder; for after the last restart. */
  forget(): void {
    // The web view may still be writing its cache for a moment after the app ended.
    rmSync(this.data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
}

function appRunning(): boolean {
  if (process.platform === "win32") return spawnSync("tasklist", ["/FI", "IMAGENAME eq buzzdome.exe", "/NH"], { encoding: "utf8" }).stdout.includes("buzzdome.exe");
  return spawnSync("pgrep", ["-x", "buzzdome"]).status === 0;
}

function killApp(): void {
  if (process.platform === "win32") spawnSync("taskkill", ["/F", "/IM", "buzzdome.exe"]);
  else spawnSync("pkill", ["-KILL", "-x", "buzzdome"]);
}
