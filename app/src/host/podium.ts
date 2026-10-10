// The final podium: a block for each of the first three places, as the game
// ranks the players (`Game::ranking`), so players who share a place stand
// on one block and a place the game skips stays empty.

import type { PlayerView } from "../types";

/** The podium's places, left to right: second, first, third. */
const PODIUM_PLACES = [2, 1, 3] as const;

/** The most names a block holds; when more share its place, one line counts the rest. */
export const MAX_PODIUM_NAMES = 3;

export interface PodiumBlock {
  place: (typeof PODIUM_PLACES)[number];
  /** The players named on it, all ranked at its place. */
  named: PlayerView[];
  /** How many more are ranked at its place than are named. */
  more: number;
}

/**
 * Who stands on each block, left to right: the players ranked at its place.
 * With first place shared (1, 1, 3), both winners stand on the first block
 * and the second stays empty, as the game skips that place.
 */
export function podiumBlocks(players: readonly PlayerView[]): PodiumBlock[] {
  return PODIUM_PLACES.map((place) => {
    const there = players.filter((p) => p.rank === place);
    const named = there.length > MAX_PODIUM_NAMES ? there.slice(0, MAX_PODIUM_NAMES - 1) : there;
    return { place, named, more: there.length - named.length };
  });
}
