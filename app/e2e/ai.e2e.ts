// Quizzes written by AI assistants (docs/ai-assistants.md), end to end: the
// real app started as an AI app's MCP server (`buzzdome --mcp`) gives the
// quizmaster's instructions, writes and saves a quiz file in the folder they
// chose, the quiz file opens in the app and plays, and the editor's File ›
// AI assistants page connects AI apps and keeps the instructions.

import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, test } from "vitest";
import { App, APP, dataFolder, t, waitFor } from "./harness";

const WINDOWS = process.platform === "win32";
const DEMO = fileURLToPath(new URL("../../samples/demo-quiz.json", import.meta.url));
const DEMO_TITLE = (JSON.parse(readFileSync(DEMO, "utf8")) as { title: string }).title;

const data = dataFolder();
/** The folder the assistant may work in: the only MCP root, the default one (Buzzdome in the documents). */
const work = join(data, "Documents", "Buzzdome");
mkdirSync(work, { recursive: true });
// Linux finds the documents folder through the XDG user dirs: point it at the test's.
mkdirSync(join(data, "config"), { recursive: true });
writeFileSync(join(data, "config", "user-dirs.dirs"), 'XDG_DOCUMENTS_DIR="$HOME/Documents"\n');
/** The AI settings (`quiz_ai::settings`), in the app's data folder (tauri.conf.json `identifier`). */
const SETTINGS = WINDOWS ? join(process.env.APPDATA ?? "", "cz.kodajek.quizshow", "ai.json") : join(data, "share", "cz.kodajek.quizshow", "ai.json");

/** An MCP client over the server's standard input and output (JSON-RPC, one message a line). */
class McpClient {
  private next = 1;
  private readonly waiting = new Map<number, (reply: Record<string, unknown>) => void>();
  readonly stderr: string[] = [];
  private constructor(private readonly server: ChildProcessWithoutNullStreams) {
    createInterface({ input: server.stdout }).on("line", (line) => {
      const reply = JSON.parse(line) as Record<string, unknown>;
      this.waiting.get(reply.id as number)?.(reply);
      this.waiting.delete(reply.id as number);
    });
    server.stderr.on("data", (chunk: Buffer) => this.stderr.push(chunk.toString()));
  }

  /** Starts `command` with `args` as an assistant starts its MCP server. */
  static start(command: string, args: string[]): McpClient {
    // The environment the app's own E2E launches use on Linux: data folders under the test's.
    const env = WINDOWS ? process.env : { ...process.env, HOME: data, XDG_DATA_HOME: join(data, "share"), XDG_CONFIG_HOME: join(data, "config"), XDG_CACHE_HOME: join(data, "cache") };
    return new McpClient(spawn(command, args, { env, stdio: ["pipe", "pipe", "pipe"] }));
  }

  request(method: string, params: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
    const id = this.next++;
    const reply = new Promise<Record<string, unknown>>((done, fail) => {
      const timer = setTimeout(() => fail(new Error(`no reply to ${method}; stderr: ${this.stderr.join("")}`)), 20_000);
      this.waiting.set(id, (answer) => {
        clearTimeout(timer);
        done(answer);
      });
    });
    this.server.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    return reply;
  }

  notify(method: string): void {
    this.server.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method })}\n`);
  }

  /** Calls tool `name`; gives its text and whether it failed. */
  async tool(name: string, args: Record<string, unknown>): Promise<{ text: string; isError: boolean }> {
    const reply = await this.request("tools/call", { name, arguments: args });
    if (reply.error) throw new Error(`${name}: ${JSON.stringify(reply.error)}`);
    const result = reply.result as { content: { type: string; text?: string }[]; isError: boolean };
    return { text: result.content.map((block) => block.text ?? "").join("\n"), isError: result.isError };
  }

  /** Closes the server's input, as the assistant does when it quits; gives the exit code. */
  close(): Promise<number | null> {
    const exited = new Promise<number | null>((done) => this.server.once("exit", (code) => done(code)));
    this.server.stdin.end();
    return exited;
  }
}

let app: App | null = null;

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

describe("AI assistants write quizzes", () => {
  const folder = join(work, "demo");
  const file = join(work, "Demo.quiz");

  test("the app as an MCP server gives the quizmaster's instructions and writes, checks and saves a quiz file, only in its folder", async () => {
    // The quizmaster chose the folder and wrote instructions in the app (File › AI assistants).
    mkdirSync(dirname(SETTINGS), { recursive: true });
    writeFileSync(SETTINGS, JSON.stringify({ folder: work, instructions: "Two rounds of five questions." }));
    const server = McpClient.start(APP, ["--mcp"]);
    const hello = await server.request("initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "e2e", version: "1" } });
    const result = hello.result as { serverInfo: { name: string }; instructions: string };
    expect(result.serverInfo.name).toBe("buzzdome");
    expect(result.instructions).toContain(work);
    server.notify("notifications/initialized");
    const listed = await server.request("tools/list");
    const tools = (listed.result as { tools: { name: string }[] }).tools.map((tool) => tool.name);
    expect(tools).toEqual(expect.arrayContaining(["quiz_format", "write_quiz", "check_quiz", "save_quiz_file", "unpack_quiz_file"]));

    const format = await server.tool("quiz_format", {});
    expect(format.isError).toBe(false);
    expect(format.text).toContain("Two rounds of five questions.");
    expect(format.text).toContain("schemaVersion");

    const written = await server.tool("write_quiz", { folder, quiz: JSON.parse(readFileSync(DEMO, "utf8")) });
    expect(written.isError, written.text).toBe(false);
    expect(written.text).toContain(DEMO_TITLE);
    const checked = await server.tool("check_quiz", { path: folder });
    expect(checked.isError, checked.text).toBe(false);
    expect(JSON.parse(checked.text.slice(0, checked.text.lastIndexOf("}") + 1)).problems).toEqual([]);
    const saved = await server.tool("save_quiz_file", { folder, file });
    expect(saved.isError, saved.text).toBe(false);
    expect(readFileSync(file).subarray(0, 2).toString("latin1")).toBe("PK");

    // Outside its folder the assistant can do nothing, and a file is never replaced unasked.
    const outside = await server.tool("write_quiz", { folder: join(data, "elsewhere"), quiz: JSON.parse(readFileSync(DEMO, "utf8")) });
    expect(outside.isError).toBe(true);
    expect(existsSync(join(data, "elsewhere"))).toBe(false);
    const again = await server.tool("save_quiz_file", { folder, file });
    expect(again.isError).toBe(true);
    const unpacked = await server.tool("unpack_quiz_file", { file, folder: join(work, "unpacked") });
    expect(unpacked.isError, unpacked.text).toBe(false);
    expect(existsSync(join(work, "unpacked", "quiz.json"))).toBe(true);

    expect(await server.close()).toBe(0);
    rmSync(SETTINGS);
  });

  // The quizmaster opens the assistant's file with File › Open: the system's picker (`App.pickFile`).
  test.skipIf(process.platform !== "linux")("the assistant's quiz file opens in the app and plays", async () => {
    app = await App.launch(data);
    await app.press("openQuiz");
    await app.pickFile(file);
    const view = await app.waitView("the assistant's quiz", (v) => v.phase.phase === "lobby");
    expect(view.quizTitle).toBe(DEMO_TITLE);
    await app.step("next");
    await app.waitView("its first slide", (v) => v.slideIndex === 0 && v.phase.phase !== "lobby");
    await app.answerQuestions(true);
    await app.press("endShow");
    await waitFor("the show ended", async () => (await app!.view()) === null);
    await app.assertHealthy();
  });

  test("the editor's AI assistants page connects AI apps where they are installed and keeps the instructions", async () => {
    app ??= await App.launch(data);
    await app.press("quizEditor");
    await app.$(".studio, .backstage");
    if ((await app.count(".backstage")) === 0) await app.press("ribbonFile");
    await app.press({ text: t("backstageAi") });
    await app.seeText(t("aiAssistantsHint"));
    if (WINDOWS) {
      // The AI apps' settings on Windows are the machine's own: only look.
      await app.seeText(t("aiInstructionsTitle"));
      await app.assertHealthy();
      return;
    }
    await app.seeText(work);
    // No Claude Desktop on a test machine: it cannot be connected.
    expect(await app.page.enabled(await app.$('[data-assistant="claudeDesktop"]'))).toBe(false);
    // Codex is connected in its configuration under the test's home.
    await app.page.click(await app.$('[data-assistant="codex"]'));
    await app.seeText(t("aiConnected", { app: "Codex", folder: work }));
    const codex = readFileSync(join(process.env.CODEX_HOME ?? join(data, ".codex"), "config.toml"), "utf8");
    expect(codex).toContain("[mcp_servers.buzzdome]");
    expect(codex).toContain('"--mcp"');
    await app.seeText(t("aiConnectedState"));
    // Another app gets the same settings to paste.
    await app.press("aiCopyConfig");
    await app.seeText('"mcpServers"');
    // The quizmaster's own instructions are kept for the server, and the default ones come back.
    const instructions = await app.$(".ai-instructions");
    await app.page.clear(instructions);
    await app.page.type(instructions, "Kids' quizzes, easy questions.");
    await app.press("aiSaveInstructions");
    await app.seeText(t("aiInstructionsSaved"));
    expect(JSON.parse(readFileSync(SETTINGS, "utf8")).instructions).toBe("Kids' quizzes, easy questions.");
    await app.press("aiDefaultInstructions");
    await waitFor("the default instructions", async () => JSON.parse(readFileSync(SETTINGS, "utf8")).instructions === null);
    expect(await app.errors()).toEqual([]);
    await app.assertHealthy();
  });
});
