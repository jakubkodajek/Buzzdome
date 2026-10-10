import { afterEach, describe, expect, it, vi } from "vitest";
import { MediaLengthTimeout, readMediaLength } from "./media";

/** A media element that, as a real one, forgets the file's length when it lets go of the file. */
class FakeMedia {
  preload = "";
  duration = Number.NaN;
  onloadedmetadata: (() => void) | null = null;
  onerror: (() => void) | null = null;
  src = "";
  removeAttribute(name: string) {
    if (name === "src") this.src = "";
  }
  load() {
    this.duration = this.src ? this.duration : Number.NaN;
  }
  /** The file's metadata arrives. */
  tell(seconds: number) {
    this.duration = seconds;
    this.onloadedmetadata?.();
  }
}

function fakeMedia(): FakeMedia {
  const media = new FakeMedia();
  vi.stubGlobal("document", { createElement: () => media });
  return media;
}

describe("readMediaLength", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("tells the length the file's metadata gives, in ms, and lets go of the file", async () => {
    const media = fakeMedia();
    const length = readMediaLength("/m/applause.wav", "audio", 1000);
    expect(media.preload).toBe("metadata");
    expect(media.src).toBe("/m/applause.wav");
    media.tell(12);
    await expect(length).resolves.toBe(12_000);
    expect(media.src).toBe("");
    expect(media.onloadedmetadata).toBeNull();
  });

  it("tells none for a file that cannot be read or tells no length", async () => {
    const unreadable = fakeMedia();
    const failed = readMediaLength("/m/broken.wav", "audio", 1000);
    unreadable.onerror?.();
    await expect(failed).resolves.toBeNaN();

    const streamed = fakeMedia();
    const unknown = readMediaLength("/m/stream.webm", "video", 1000);
    streamed.tell(Number.POSITIVE_INFINITY);
    await expect(unknown).resolves.toBeNaN();
  });

  it("gives up on a file that does not tell in time", async () => {
    vi.useFakeTimers();
    const media = fakeMedia();
    const length = readMediaLength("/m/slow.wav", "audio", 1000);
    vi.advanceTimersByTime(1000);
    await expect(length).rejects.toBeInstanceOf(MediaLengthTimeout);
    expect(media.src).toBe("");
  });
});
