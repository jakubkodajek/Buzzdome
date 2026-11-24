// The AI panel's conversation (AiPanel.tsx): what the quizmaster and the
// model said, and what of it the model gets with the next message or the
// request for the questions.

import type { ChatTurn, Source } from "../types";

/** One entry of the conversation as the panel shows it. */
export type ChatEntry =
  /** `files`: the ids of the panel's files sent with it (`AiFile.id`), shown in its bubble rather than the message box. */
  | { id: number; role: "user"; text: string; files: number[] }
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

/** The files of `files` not sent with a message yet: the ones the message box shows. */
export function unsent<F extends { id: number }>(files: readonly F[], entries: readonly ChatEntry[]): F[] {
  const sent = new Set(entries.flatMap((shown) => (shown.role === "user" ? shown.files : [])));
  return files.filter((file) => !sent.has(file.id));
}

/** Whether a message may be sent: words typed, or new files (`files` not sent yet) to talk about. */
export function canSend(message: string, files: number): boolean {
  return message.trim() !== "" || files > 0;
}

/** Whether the questions may be made: from words typed, the conversation, or the files alone. */
export function canCreate(message: string, files: number, entries: readonly ChatEntry[]): boolean {
  return message.trim() !== "" || files > 0 || turnsOf(entries).some((turn) => turn.role === "user");
}

/**
 * The conversation without the quizmaster's message `id` when it is still
 * the last entry, unanswered (the model failed or was cancelled), and the
 * message box with its words again when nothing new was typed meanwhile;
 * null when it was answered or other entries came after it.
 */
export function takeBack(entries: readonly ChatEntry[], id: number, message: string): { entries: ChatEntry[]; message: string } | null {
  const last = entries[entries.length - 1];
  if (last?.id !== id || last.role !== "user") return null;
  return { entries: entries.slice(0, -1), message: message.trim() === "" ? last.text : message };
}

/** A piece of a line of the model's reply: plain, or bold (`**...**`). */
export interface Run {
  text: string;
  bold: boolean;
}

/** A block of the model's reply as the panel shows it. */
export type ReplyBlock =
  | { kind: "paragraph"; lines: Run[][] }
  | { kind: "heading"; runs: Run[] }
  | { kind: "list"; ordered: boolean; start: number; items: Run[][] };

/** `line` split into plain and bold runs (`**bold**`); a lone `**` stays as it is. */
export function runsOf(line: string): Run[] {
  const runs: Run[] = [];
  const pattern = /\*\*(.+?)\*\*/g;
  let at = 0;
  for (const found of line.matchAll(pattern)) {
    if (found.index > at) runs.push({ text: line.slice(at, found.index), bold: false });
    runs.push({ text: found[1], bold: true });
    at = found.index + found[0].length;
  }
  if (at < line.length) runs.push({ text: line.slice(at), bold: false });
  return runs;
}

const BULLET = /^\s*[-*•]\s+(.*)$/;
const NUMBERED = /^\s*(\d{1,3})[.)]\s+(.*)$/;
const HEADING = /^\s*#{1,6}\s+(.*)$/;

/**
 * The model's reply in blocks: paragraphs, headings (`#`), bulleted and
 * numbered lists, the Markdown that local models write, read leniently;
 * anything else stays plain text.
 */
export function replyBlocks(text: string): ReplyBlock[] {
  const blocks: ReplyBlock[] = [];
  for (const raw of text.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trimEnd();
    const last = blocks[blocks.length - 1];
    const bullet = BULLET.exec(line);
    const numbered = NUMBERED.exec(line);
    const heading = HEADING.exec(line);
    if (line.trim() === "") {
      blocks.push({ kind: "paragraph", lines: [] });
    } else if (heading) {
      blocks.push({ kind: "heading", runs: runsOf(heading[1].replace(/\*\*/g, "")) });
    } else if (bullet || numbered) {
      const ordered = numbered !== null;
      const item = runsOf(bullet ? bullet[1] : numbered![2]);
      if (last?.kind === "list" && last.ordered === ordered) last.items.push(item);
      else blocks.push({ kind: "list", ordered, start: numbered ? Number(numbered[1]) : 1, items: [item] });
    } else if (last?.kind === "list" && /^\s+/.test(raw)) {
      // An indented line goes on with the item above it.
      last.items[last.items.length - 1].push({ text: ` ${line.trim()}`, bold: false });
    } else if (last?.kind === "paragraph") {
      last.lines.push(runsOf(line));
    } else {
      blocks.push({ kind: "paragraph", lines: [runsOf(line)] });
    }
  }
  return blocks.filter((block) => block.kind !== "paragraph" || block.lines.length > 0);
}
