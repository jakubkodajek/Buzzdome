// The AI panel's conversation (AiPanel.tsx): what the quizmaster and the
// model said, and what of it the model gets with the next message or the
// request for the questions.

import type { ChatTurn, Source } from "../types";

/** One entry of the conversation as the panel shows it. */
export type ChatEntry =
  | { id: number; role: "user"; text: string; files: string[] }
  | { id: number; role: "assistant"; text: string; cutOff: boolean; sources: Source[] }
  /** What the app says between the turns (questions inserted): the model never gets it. */
  | { id: number; role: "note"; text: string };

/** An entry before it has its id. */
export type NewEntry = ChatEntry extends infer E ? (E extends ChatEntry ? Omit<E, "id"> : never) : never;

let nextId = 1;

/** A new entry with an id of its own. */
export function entry(made: NewEntry): ChatEntry {
  return { ...made, id: nextId++ } as ChatEntry;
}

/** The turns the model gets of `entries`: the quizmaster's and its own, not the app's notes, nor a turn with no words. */
export function turnsOf(entries: readonly ChatEntry[]): ChatTurn[] {
  const turns: ChatTurn[] = [];
  for (const shown of entries) {
    if (shown.role === "note" || shown.text.trim() === "") continue;
    turns.push({ role: shown.role, text: shown.text });
  }
  return turns;
}

/**
 * Whether a message may be sent: words typed, or files to talk about when
 * nothing was said yet.
 */
export function canSend(message: string, files: number, entries: readonly ChatEntry[]): boolean {
  return message.trim() !== "" || (files > 0 && turnsOf(entries).length === 0);
}

/** Whether the questions may be made: from words typed, the conversation, or the files alone. */
export function canCreate(message: string, files: number, entries: readonly ChatEntry[]): boolean {
  return message.trim() !== "" || files > 0 || turnsOf(entries).some((turn) => turn.role === "user");
}
