import { describe, expect, it } from "vitest";
import { blockNoter, MAX_NOTED_PER_MINUTE, type Violation } from "./csp-report";

const block = (timeStamp: number, blocked = "https://example.com/x.js"): Violation => ({
  effectiveDirective: "script-src-elem",
  blockedURI: blocked,
  sourceFile: "tauri://localhost/assets/index.js",
  lineNumber: 12,
  timeStamp,
});

describe("blockNoter", () => {
  it("notes what was blocked, by which rule and where", () => {
    const lines: string[] = [];
    const noteBlock = blockNoter((line) => lines.push(line));
    noteBlock(block(0));
    noteBlock({ ...block(1, "inline"), effectiveDirective: "style-src-attr", sourceFile: "" });
    noteBlock({ ...block(2, ""), effectiveDirective: "img-src", sourceFile: "" });
    expect(lines).toEqual([
      "the content security policy blocked https://example.com/x.js (script-src-elem) at tauri://localhost/assets/index.js:12",
      "the content security policy blocked inline (style-src-attr)",
      "the content security policy blocked a resource (img-src)",
    ]);
  });

  it("notes at most a few blocks a minute and then says how many it left out", () => {
    const lines: string[] = [];
    const noteBlock = blockNoter((line) => lines.push(line));
    for (let at = 0; at < 100; at += 1) noteBlock(block(at * 100));
    expect(lines).toHaveLength(MAX_NOTED_PER_MINUTE);
    noteBlock(block(59_999));
    expect(lines).toHaveLength(MAX_NOTED_PER_MINUTE);
    noteBlock(block(60_000));
    noteBlock(block(60_001));
    expect(lines).toHaveLength(MAX_NOTED_PER_MINUTE + 2);
    expect(lines[MAX_NOTED_PER_MINUTE]).toContain(`(${101 - MAX_NOTED_PER_MINUTE} more left out before it)`);
    expect(lines[MAX_NOTED_PER_MINUTE + 1]).not.toContain("left out");
  });
});
