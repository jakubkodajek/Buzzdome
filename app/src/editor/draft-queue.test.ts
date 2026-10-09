import { describe, expect, it } from "vitest";
import { DraftQueue } from "./draft-queue";

/** A promise with its resolver, to finish backend calls when a test says so. */
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe("DraftQueue", () => {
  it("writes one value at a time and collapses waiting writes into the latest", async () => {
    const written: number[] = [];
    const gate = deferred();
    const queue = new DraftQueue<number>(async (value) => {
      written.push(value);
      if (value === 1) await gate.promise;
    });
    queue.push(0);
    queue.push(1);
    await settle();
    // 0 never started, so 1 replaced it; 1 is now being written.
    expect(written).toEqual([1]);
    queue.push(2);
    queue.push(3);
    await settle();
    expect(written).toEqual([1]);
    gate.resolve();
    await settle();
    expect(written).toEqual([1, 3]);
  });

  it("runs tasks in order with writes; a write pushed after a task comes after it", async () => {
    const log: string[] = [];
    const queue = new DraftQueue<string>(async (value) => {
      log.push(`write ${value}`);
    });
    queue.push("a");
    const task = queue.run(async () => {
      log.push("task");
      return 42;
    });
    queue.push("b");
    expect(await task).toBe(42);
    await settle();
    expect(log).toEqual(["write a", "task", "write b"]);
  });

  it("keeps going after a failed task", async () => {
    const log: string[] = [];
    const queue = new DraftQueue<string>(async (value) => {
      log.push(value);
    });
    await expect(queue.run(() => Promise.reject(new Error("disk full")))).rejects.toThrow("disk full");
    queue.push("after");
    await settle();
    expect(log).toEqual(["after"]);
  });
});
