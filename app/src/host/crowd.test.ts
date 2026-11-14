import { describe, expect, it } from "vitest";
import { chunks, LEADERBOARD_COLUMNS, LEADERBOARD_ROWS, leaderboardPages, MAX_WAITING_CELLS, queueStrip, waitingStrip } from "./crowd";

describe("waitingStrip", () => {
  it("gives every waiting buzzer a cell while they fit, in more rows as more wait", () => {
    expect(waitingStrip(0)).toEqual({ rows: 2, shown: 0, more: 0 });
    expect(waitingStrip(20)).toEqual({ rows: 2, shown: 20, more: 0 });
    expect(waitingStrip(21)).toEqual({ rows: 3, shown: 21, more: 0 });
    expect(waitingStrip(34)).toEqual({ rows: 4, shown: 34, more: 0 });
    expect(waitingStrip(MAX_WAITING_CELLS)).toEqual({ rows: 4, shown: MAX_WAITING_CELLS, more: 0 });
  });

  it("past the strip's cells counts the rest in its last cell, so no buzzer goes unmentioned", () => {
    expect(waitingStrip(MAX_WAITING_CELLS + 1)).toEqual({ rows: 4, shown: MAX_WAITING_CELLS - 1, more: 2 });
    expect(waitingStrip(100)).toEqual({ rows: 4, shown: MAX_WAITING_CELLS - 1, more: 100 - MAX_WAITING_CELLS + 1 });
  });
});

describe("leaderboardPages", () => {
  const teams = (n: number) => Array.from({ length: n }, (_, i) => i + 1);

  it("cuts a list in pieces", () => {
    expect(chunks([], 3)).toEqual([]);
    expect(chunks([1, 2, 3, 4], 3)).toEqual([[1, 2, 3], [4]]);
  });

  it("puts every team on a page, in ranking order, columns of ten", () => {
    expect(leaderboardPages(teams(8))).toEqual([[teams(8)]]);
    const pages = leaderboardPages(teams(47));
    expect(pages.map((page) => page.map((column) => column.length))).toEqual([[10, 10], [10, 10], [7]]);
    expect(pages.flat(2)).toEqual(teams(47));
    expect(LEADERBOARD_ROWS * LEADERBOARD_COLUMNS).toBe(20);
  });
});

describe("queueStrip", () => {
  const turns = (states: ("answering" | "wrong" | "right" | "waiting")[]) => states.map((state, i) => ({ buzzer: i + 1, state }));

  it("shows every buzz while they fit", () => {
    const few = turns(["wrong", "answering", "waiting"]);
    expect(queueStrip(few)).toEqual({ shown: few, more: 0 });
  });

  it("keeps the one answering, the wrong one before it and the next waiting, and counts the rest", () => {
    const { shown, more } = queueStrip(turns(["wrong", "wrong", "answering", ...Array(17).fill("waiting")]));
    expect(shown.map((turn) => turn.buzzer)).toEqual([2, 3, 4]);
    expect(more).toBe(17);
  });

  it("keeps the last one judged when none answers, and the first waiting when the first answers", () => {
    expect(queueStrip(turns(["wrong", "wrong", "wrong", "wrong", "wrong"])).shown.map((turn) => turn.buzzer)).toEqual([3, 4, 5]);
    expect(queueStrip(turns(["answering", "waiting", "waiting", "waiting", "waiting"])).shown.map((turn) => turn.buzzer)).toEqual([1, 2, 3]);
  });
});
