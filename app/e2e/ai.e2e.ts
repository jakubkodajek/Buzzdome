// Quizzes written by AI assistants (docs/ai-assistants.md), end to end: the
// real app started as an assistant's MCP server (`buzzdome --mcp`) writes
// and saves a quiz file, the `buzzdome-quiz` command line checks, packs and
// unpacks one, the quiz file opens in the app and plays, and the editor's
// File › AI assistants page saves the skill package and connects Claude
// Desktop only where it is installed.

import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { App, APP, dataFolder, t, waitFor } from "./harness";

const WINDOWS = process.platform === "win32";
/** The command line, built with `cargo build -p quiz-ai` (or BUZZDOME_QUIZ_CLI naming one). */
const CLI = process.env.BUZZDOME_QUIZ_CLI ?? fileURLToPath(new URL(`../../target/debug/buzzdome-quiz${WINDOWS ? ".exe" : ""}`, import.meta.url));
const DEMO = fileURLToPath(new URL("../../samples/demo-quiz.json", import.meta.url));
const DEMO_TITLE = (JSON.parse(readFileSync(DEMO, "utf8")) as { title: string }).title;

const data = dataFolder();
/** The folder the assistant may work in: the only MCP root. */
const work = join(data, "Documents", "Buzzdome");
mkdirSync(work, { recursive: true });
// Linux finds the documents folder through the XDG user dirs: point it at the test's.
mkdirSync(join(data, "config"), { recursive: true });
writeFileSync(join(data, "config", "user-dirs.dirs"), 'XDG_DOCUMENTS_DIR="$HOME/Documents"\n');

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

/** Runs the command line with `args`; gives its exit code and output. */
function cli(...args: string[]): { code: number | null; out: string; err: string } {
  const run = spawnSync(CLI, args, { encoding: "utf8" });
  return { code: run.status, out: run.stdout, err: run.stderr };
}

let app: App | null = null;

beforeAll(() => {
  if (!existsSync(CLI)) throw new Error(`no ${CLI}: build it with \`cargo build -p quiz-ai\``);
});

afterAll(async () => {
  await app?.quit();
  app?.forget();
});

describe("AI assistants write quizzes", () => {
  const folder = join(work, "demo");
  const file = join(work, "Demo.quiz");

  test("the app as an MCP server writes, checks and saves a quiz file, only in its folder", async () => {
    const server = McpClient.start(APP, ["--mcp", "--root", work]);
    const hello = await server.request("initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "e2e", version: "1" } });
    expect((hello.result as { serverInfo: { name: string } }).serverInfo.name).toBe("buzzdome");
    server.notify("notifications/initialized");
    const listed = await server.request("tools/list");
    const tools = (listed.result as { tools: { name: string }[] }).tools.map((tool) => tool.name);
    expect(tools).toEqual(expect.arrayContaining(["quiz_format", "write_quiz", "check_quiz", "save_quiz_file", "unpack_quiz_file"]));

    const format = await server.tool("quiz_format", {});
    expect(format.isError).toBe(false);
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

    expect(await server.close()).toBe(0);
  });

  test("the command line checks, unpacks and packs quiz files", () => {
    const checked = cli("check", file);
    expect(checked.code, checked.err).toBe(0);
    expect(checked.out).toContain(DEMO_TITLE);

    const unpacked = join(work, "unpacked");
    expect(cli("unpack", file, unpacked).code).toBe(0);
    expect(existsSync(join(unpacked, "quiz.json"))).toBe(true);
    const repacked = join(work, "Repacked.quiz");
    expect(cli("pack", unpacked, repacked).code).toBe(0);
    expect(cli("check", repacked).code).toBe(0);
    // An existing file is replaced only with --replace.
    expect(cli("pack", unpacked, repacked).code).not.toBe(0);
    expect(cli("pack", unpacked, repacked, "--replace").code).toBe(0);

    // A quiz with a problem fails the check, with the problem said.
    const broken = join(work, "broken");
    mkdirSync(broken);
    writeFileSync(join(broken, "quiz.json"), "{ not json");
    const failed = cli("check", broken);
    expect(failed.code).not.toBe(0);
    expect(failed.err.length).toBeGreaterThan(0);

    expect(JSON.parse(cli("schema").out).properties.schemaVersion).toBeDefined();
    const skill = join(work, "skill.zip");
    expect(cli("skill", skill).code).toBe(0);
    expect(readFileSync(skill).subarray(0, 2).toString("latin1")).toBe("PK");
    expect(cli("no-such-command").code).toBe(2);
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

  test("the editor's AI assistants page connects Claude Desktop only where it is installed, and saves the skill", async () => {
    app ??= await App.launch(data);
    await app.press("quizEditor");
    await app.$(".studio, .backstage");
    if ((await app.count(".backstage")) === 0) await app.press("ribbonFile");
    await app.press({ text: t("backstageAi") });
    await app.seeText(t("aiAssistantsHint"));
    // No Claude Desktop on a test machine: the app says so and changes nothing.
    await app.press("connectClaude");
    await app.seeText(t("errClaudeDesktopMissing"));
    expect(await app.errors()).toEqual([]);
    if (process.platform === "linux") {
      const skill = join(data, "from-the-editor.zip");
      await app.press("saveAiSkill");
      await app.pickFile(skill);
      await waitFor("the skill package", async () => existsSync(skill) && readFileSync(skill).length > 0);
      expect(readFileSync(skill).subarray(0, 2).toString("latin1")).toBe("PK");
      await app.seeText(t("aiSkillSaved"));
    }
    await app.assertHealthy();
  });
});
