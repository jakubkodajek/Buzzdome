import { describe, expect, it } from "vitest";
import { mediaDurations, type TimedKind } from "./media-durations";

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A reader whose answers the test gives: which files it was asked for, and a way to answer the oldest. */
function reader() {
  const asked: string[] = [];
  const answers: ((ms: number) => void)[] = [];
  const read = (url: string) => {
    asked.push(url);
    return new Promise<number>((resolve) => answers.push(resolve));
  };
  /** Lets the queue reach the reader, answers the oldest open question and lets the queue move on. */
  const answer = async (ms: number) => {
    await settle();
    answers.shift()?.(ms);
    await settle();
  };
  return { read, asked, answer };
}

const files = (...urls: string[]) => new Map(urls.map((url): [string, TimedKind] => [url, url.endsWith(".mp4") ? "video" : "audio"]));

describe("media lengths", () => {
  it("are read one file after another and shown as each becomes known", async () => {
    const { read, asked, answer } = reader();
    const durations = mediaDurations(read);
    let changes = 0;
    durations.lengths.subscribe(() => (changes += 1));
    durations.follow(files("a.mp3", "b.mp4"));
    await settle();
    expect(asked).toEqual(["a.mp3"]);
    await answer(1000);
    expect([...durations.lengths.get()]).toEqual([["a.mp3", 1000]]);
    expect(asked).toEqual(["a.mp3", "b.mp4"]);
    await answer(Number.NaN);
    expect(durations.lengths.get().get("b.mp4")).toBeNaN();
    expect(changes).toBe(2);
  });

  it("are read once per file however often the list comes", async () => {
    const { read, asked, answer } = reader();
    const durations = mediaDurations(read);
    durations.follow(files("a.mp3"));
    durations.follow(files("a.mp3"));
    await answer(1000);
    durations.follow(files("a.mp3"));
    await settle();
    expect(asked).toEqual(["a.mp3"]);
  });

  it("forget a file that leaves the list, so one of the same name is read again", async () => {
    const { read, asked, answer } = reader();
    const durations = mediaDurations(read);
    durations.follow(files("a.mp3", "b.mp3"));
    await answer(1000);
    await answer(2000);
    const before = durations.lengths.get();
    durations.follow(files("b.mp3"));
    expect([...durations.lengths.get()]).toEqual([["b.mp3", 2000]]);
    // A new map, so a component that compares it draws again.
    expect(durations.lengths.get()).not.toBe(before);
    durations.follow(files("a.mp3", "b.mp3"));
    await answer(1500);
    expect(asked).toEqual(["a.mp3", "b.mp3", "a.mp3"]);
    expect(durations.lengths.get().get("a.mp3")).toBe(1500);
  });

  it("skip a file that left the list before its turn and do not keep one that left while it was read", async () => {
    const { read, asked, answer } = reader();
    const durations = mediaDurations(read);
    durations.follow(files("a.mp3", "b.mp3", "c.mp3"));
    await settle();
    durations.follow(files("c.mp3"));
    await answer(1000);
    await answer(3000);
    expect(asked).toEqual(["a.mp3", "c.mp3"]);
    expect([...durations.lengths.get()]).toEqual([["c.mp3", 3000]]);
  });

  it("change nothing for a list that drops nothing new", () => {
    const durations = mediaDurations(reader().read);
    const before = durations.lengths.get();
    durations.follow(files());
    expect(durations.lengths.get()).toBe(before);
  });
});
