import { describe, expect, it } from "vitest";
import type { PlayerView } from "../types";
import { MAX_PODIUM_NAMES, podiumBlocks } from "./podium";

const player = (buzzer: number, rank: number | null) => ({ buzzer, rank }) as PlayerView;
const standing = (players: PlayerView[]) =>
  podiumBlocks(players).map(({ place, named, more }) => [place, named.map((p) => p.buzzer), more]);

describe("podiumBlocks", () => {
  it("puts each of the first three places on its block, second, first and third from the left", () => {
    expect(standing([player(1, 1), player(2, 2), player(3, 3), player(4, 4)])).toEqual([
      [2, [2], 0],
      [1, [1], 0],
      [3, [3], 0],
    ]);
  });

  it("stands players who share a place on one block, and leaves a skipped place empty", () => {
    expect(standing([player(1, 1), player(2, 1), player(3, 3)])).toEqual([
      [2, [], 0],
      [1, [1, 2], 0],
      [3, [3], 0],
    ]);
    expect(standing([player(1, 1), player(2, 2), player(3, 3), player(4, 3)])).toEqual([
      [2, [2], 0],
      [1, [1], 0],
      [3, [3, 4], 0],
    ]);
  });

  it("names at most a few on a block and counts the rest", () => {
    const tied = [1, 2, 3, 4, 5].map((buzzer) => player(buzzer, 1));
    expect(standing(tied)).toEqual([
      [2, [], 0],
      [1, [1, 2], 3],
      [3, [], 0],
    ]);
    expect(podiumBlocks(tied.slice(0, MAX_PODIUM_NAMES))[1].named).toHaveLength(MAX_PODIUM_NAMES);
  });

  it("leaves out players switched off, who have no place", () => {
    expect(standing([player(1, null), player(2, 1)])).toEqual([
      [2, [], 0],
      [1, [2], 0],
      [3, [], 0],
    ]);
  });
});
