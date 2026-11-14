// Buzzers that join a show once its slides began. A press from any buzzer
// the show does not know adds a team (`Game::press`, specification SF-4),
// and the Enjoy 303S buzzers have no power switch: a spare buzzer pressed
// by mistake becomes a team that never answers, so a question no longer
// ends by itself once everyone answered. The Director tells the quizmaster
// at once, unless the buzzer joined from this window (added in Teams,
// pressed on screen or with a key), which was meant.

import type { Command, GameView } from "./types";

/** The buzzers this window sent a press for or added, in the show it last saw. */
const sentHere = new Set<number>();

/** Notes a game command this window sends (`useDispatch`). */
export function noteSent(command: Command): void {
  if (command.type === "press" || command.type === "addPlayer") sentHere.add(command.buzzer);
}

/** Forgets what `noteSent` noted, for a show that ended or a new one. */
export function forgetSent(): void {
  sentHere.clear();
}

/**
 * The buzzers that joined between `before` and `after`, two views of the
 * same show, once its slides began; not those that joined from this window
 * (`noteSent`). None when `after` is another show, whose step count starts
 * again.
 */
export function lateJoins(before: GameView | null, after: GameView | null, here: ReadonlySet<number> = sentHere): number[] {
  if (!before || !after || after.stepNo < before.stepNo) return [];
  if (after.phase.phase === "welcome" || after.phase.phase === "lobby") return [];
  const known = new Set(before.players.map((p) => p.buzzer));
  return after.players.map((p) => p.buzzer).filter((buzzer) => !known.has(buzzer) && !here.has(buzzer));
}
