// A small W3C WebDriver client: just the calls the end-to-end tests make,
// over fetch, so the tests need no driver library.
// https://www.w3.org/TR/webdriver2/

/** The key W3C WebDriver puts an element reference under. */
const ELEMENT = "element-6066-11e4-a52e-4f735466cecf";

export type ElementRef = { [ELEMENT]: string };

/** Keys as WebDriver names them (https://www.w3.org/TR/webdriver2/#keyboard-actions). */
export const Key = {
  Enter: "\uE007",
  Escape: "\uE00C",
  Space: " ",
  ArrowRight: "\uE014",
  Control: "\uE009",
  Shift: "\uE008",
  F5: "\uE035",
} as const;

export class WebDriverError extends Error {
  constructor(
    readonly error: string,
    message: string,
  ) {
    super(`${error}: ${message}`);
  }
}

export class Session {
  private constructor(
    private readonly base: string,
    readonly id: string,
  ) {}

  /** Starts a session on the driver at `base` with `capabilities`. */
  static async start(base: string, capabilities: Record<string, unknown>): Promise<Session> {
    const value = await call<{ sessionId: string }>(base, "POST", "/session", { capabilities: { alwaysMatch: capabilities } });
    return new Session(base, value.sessionId);
  }

  private send<T>(method: string, path: string, body?: unknown): Promise<T> {
    return call<T>(this.base, method, `/session/${this.id}${path}`, body);
  }

  async end(): Promise<void> {
    await this.send("DELETE", "");
  }

  windowHandles(): Promise<string[]> {
    return this.send("GET", "/window/handles");
  }

  windowHandle(): Promise<string> {
    return this.send("GET", "/window");
  }

  async switchToWindow(handle: string): Promise<void> {
    await this.send("POST", "/window", { handle });
  }

  title(): Promise<string> {
    return this.send("GET", "/title");
  }

  /** Every element `css` matches, in document order. */
  findAll(css: string): Promise<ElementRef[]> {
    return this.send("POST", "/elements", { using: "css selector", value: css });
  }

  findAllXPath(xpath: string): Promise<ElementRef[]> {
    return this.send("POST", "/elements", { using: "xpath", value: xpath });
  }

  async click(element: ElementRef): Promise<void> {
    await this.send("POST", `/element/${element[ELEMENT]}/click`, {});
  }

  /** Clicks `element` twice at once with the mouse, as a double click does. */
  async doubleClick(element: ElementRef): Promise<void> {
    const press = [
      { type: "pointerDown", button: 0 },
      { type: "pointerUp", button: 0 },
    ];
    const actions = [{ type: "pointerMove", origin: element, x: 0, y: 0 }, ...press, ...press];
    await this.send("POST", "/actions", { actions: [{ type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions }] });
    await this.send("DELETE", "/actions");
  }

  text(element: ElementRef): Promise<string> {
    return this.send("GET", `/element/${element[ELEMENT]}/text`);
  }

  attribute(element: ElementRef, name: string): Promise<string | null> {
    return this.send("GET", `/element/${element[ELEMENT]}/attribute/${name}`);
  }

  displayed(element: ElementRef): Promise<boolean> {
    return this.send("GET", `/element/${element[ELEMENT]}/displayed`);
  }

  enabled(element: ElementRef): Promise<boolean> {
    return this.send("GET", `/element/${element[ELEMENT]}/enabled`);
  }

  async clear(element: ElementRef): Promise<void> {
    await this.send("POST", `/element/${element[ELEMENT]}/clear`, {});
  }

  async type(element: ElementRef, text: string): Promise<void> {
    await this.send("POST", `/element/${element[ELEMENT]}/value`, { text });
  }

  /** Runs `script` (a function body) in the page with `args`; gives what it returns. */
  execute<T>(script: string, args: unknown[] = []): Promise<T> {
    return this.send("POST", "/execute/sync", { script, args });
  }

  /** Runs `script` (a function body whose last argument is a callback) and gives what it passes the callback. */
  executeAsync<T>(script: string, args: unknown[] = []): Promise<T> {
    return this.send("POST", "/execute/async", { script, args });
  }

  /** Presses `keys` one after another, each down and up, as a person types them, with `held` held down. */
  async keys(keys: string[], held: string[] = []): Promise<void> {
    const down = (value: string) => ({ type: "keyDown", value });
    const up = (value: string) => ({ type: "keyUp", value });
    const actions = [...held.map(down), ...keys.flatMap((key) => [down(key), up(key)]), ...held.map(up)];
    await this.send("POST", "/actions", { actions: [{ type: "key", id: "keyboard", actions }] });
    await this.send("DELETE", "/actions");
  }

  async screenshot(): Promise<string> {
    return this.send("GET", "/screenshot");
  }
}

async function call<T>(base: string, method: string, path: string, body?: unknown): Promise<T> {
  const response = await fetch(base + path, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const payload = (await response.json().catch(() => ({ value: null }))) as { value: T & { error?: string; message?: string } };
  if (!response.ok || (payload.value && typeof payload.value === "object" && "error" in payload.value && payload.value.error)) {
    const value = payload.value ?? {};
    throw new WebDriverError(value.error ?? `HTTP ${response.status}`, value.message ?? `${method} ${path}`);
  }
  return payload.value;
}
