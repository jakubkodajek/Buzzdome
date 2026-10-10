import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { inSpan, linearGain, PartPlayer, sameSpan, WHOLE_FILE } from "./clip-player";

/** Just enough of a media element for a part player: a position, play, pause and events. */
class FakeMedia extends EventTarget {
  currentTime = 0;
  duration = 60;
  paused = true;
  play() {
    this.paused = false;
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  }
  pause() {
    this.paused = true;
    this.dispatchEvent(new Event("pause"));
  }
  /** Plays on to `seconds`, as the element would, and reports the end of the file. */
  playTo(seconds: number) {
    this.currentTime = Math.min(seconds, this.duration);
    if (seconds >= this.duration) this.dispatchEvent(new Event("ended"));
    vi.advanceTimersByTime(100);
  }
}

describe("clip player", () => {
  it("multiplies volume and loudness gain", () => {
    expect(linearGain(100, 0)).toBe(1);
    expect(linearGain(50, 0)).toBe(0.5);
    // +6.02 dB doubles the amplitude, -20 dB is a tenth.
    expect(linearGain(100, 6.0206)).toBeCloseTo(2, 4);
    expect(linearGain(80, -20)).toBeCloseTo(0.08, 6);
    expect(linearGain(0, 20)).toBe(0);
  });

  it("knows which times a part covers", () => {
    const chorus = { fromMs: 15_000, toMs: 45_000 };
    expect([14_999, 15_000, 44_999, 45_000].map((ms) => inSpan(chorus, ms))).toEqual([false, true, true, false]);
    expect(inSpan(WHOLE_FILE, 3_600_000)).toBe(true);
    expect(sameSpan(chorus, { fromMs: 15_000, toMs: 45_000 })).toBe(true);
    expect(sameSpan(chorus, WHOLE_FILE)).toBe(false);
  });
});

describe("part player", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  const setUp = () => {
    const media = new FakeMedia();
    return { media, player: new PartPlayer(media as unknown as HTMLMediaElement) };
  };
  const part = { fromMs: 5_000, toMs: 20_000 };

  it("plays a part to its end, then stands at its start and knows it finished", () => {
    const { media, player } = setUp();
    player.play(part);
    expect([media.currentTime, player.playing, player.finished]).toEqual([5, true, false]);
    media.playTo(20);
    expect([media.currentTime, player.playing, player.finished]).toEqual([5, false, true]);
    // Playing again starts over.
    player.play(part);
    expect(player.finished).toBe(false);
  });

  it("goes on where it was paused, and loops a part until stopped", () => {
    const { media, player } = setUp();
    player.play(part, { loop: true });
    media.playTo(12);
    player.pause();
    player.play(part, { loop: true });
    expect(media.currentTime).toBe(12);
    media.playTo(20);
    expect([media.currentTime, player.playing, player.finished]).toEqual([5, true, false]);
    player.stop();
    expect([media.currentTime, player.playing]).toEqual([5, false]);
  });

  it("ends a part that runs to the end of the file when the file ends", () => {
    const { media, player } = setUp();
    player.play({ fromMs: 50_000, toMs: null });
    media.playTo(60);
    expect([player.playing, player.finished]).toEqual([false, true]);
  });
});
