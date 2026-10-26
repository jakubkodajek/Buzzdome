import { describe, expect, it } from "vitest";
import { previewView } from "../host/preview";
import { newSlide } from "../editor/slides";
import type { NowPlaying, VideoClip } from "../types";
import { otherSoundPlays } from "./show-sounds";

describe("show music", () => {
  const slide = newSlide("question", "q1");
  const view = previewView(slide, 0, 1, "Quiz");
  const playing = (kind: NowPlaying["kind"], sinceMs: number | null): NowPlaying => ({ id: 1, reference: "media/x", kind, positionMs: 0, sinceMs });

  it("makes way for the slide's sound, the media player's sound or video, and a slide video with sound", () => {
    expect(otherSoundPlays(view, false, null)).toBe(false);
    expect(otherSoundPlays(view, true, null)).toBe(true);
    expect(otherSoundPlays(view, false, playing("audio", 0))).toBe(true);
    expect(otherSoundPlays(view, false, playing("video", 0))).toBe(true);
    // A picture has no sound, and a paused sound plays none.
    expect(otherSoundPlays(view, false, playing("image", 0))).toBe(false);
    expect(otherSoundPlays(view, false, playing("audio", null))).toBe(false);
    const video = { file: "media/clip.mp4", muted: false, volume: 100 } as VideoClip;
    expect(otherSoundPlays({ ...view, slide: { ...slide, video } }, false, null)).toBe(true);
    expect(otherSoundPlays({ ...view, slide: { ...slide, video: { ...video, muted: true } } }, false, null)).toBe(false);
  });
});
