import { describe, expect, it } from "vitest";
import { addFiles, aiFileKind, fitWithin, frameTimes, hasSound, resample } from "./ai-files";

const file = (name: string, size = 10, lastModified = 1) => new File([new Uint8Array(size)], name, { lastModified });

describe("the AI panel's files", () => {
  it("takes pictures, videos, sound recordings and text files by their names", () => {
    expect(["a.JPG", "b.png", "c.webp"].map(aiFileKind)).toEqual(["picture", "picture", "picture"]);
    expect(["d.mp4", "e.webm"].map(aiFileKind)).toEqual(["video", "video"]);
    expect(["f.txt", "g.MD", "h.csv"].map(aiFileKind)).toEqual(["text", "text", "text"]);
    expect(["m.mp3", "n.WAV", "o.m4a"].map(aiFileKind)).toEqual(["recording", "recording", "recording"]);
    expect(["i.pdf", "j.exe", "k", "l.docx"].map(aiFileKind)).toEqual([null, null, null, null]);
  });

  it("writes down the sound of recordings and videos, resampled to the rate the listener hears", () => {
    expect((["recording", "video", "picture", "text"] as const).map(hasSound)).toEqual([true, true, false, false]);
    const halved = resample(new Float32Array([0, 1, 2, 3, 4, 5, 6, 7]), 32_000, 16_000);
    expect([...halved]).toEqual([0, 2, 4, 6]);
    const doubled = resample(new Float32Array([0, 2, 4]), 8_000, 16_000);
    expect([...doubled]).toEqual([0, 1, 2, 3, 4, 4]);
  });

  it("adds each file once, up to the most a request takes, and names the ones it cannot read", () => {
    const first = addFiles([], [file("a.jpg"), file("notes.pdf"), file("a.jpg")], 3);
    expect(first.files.map((f) => f.file.name)).toEqual(["a.jpg"]);
    expect(first.unsupported).toEqual(["notes.pdf"]);
    expect(first.tooMany).toBe(false);
    // The same name with other contents is another file.
    const second = addFiles(first.files, [file("a.jpg", 20), file("b.mp4"), file("c.txt")], 3);
    expect(second.files.map((f) => f.kind)).toEqual(["picture", "picture", "video"]);
    expect(second.tooMany).toBe(true);
    expect(new Set(second.files.map((f) => f.id)).size).toBe(3);
  });

  it("makes pictures smaller, never larger", () => {
    expect(fitWithin(4000, 3000, 1024)).toEqual({ width: 1024, height: 768 });
    expect(fitWithin(1080, 1920, 1024)).toEqual({ width: 576, height: 1024 });
    expect(fitWithin(300, 200, 1024)).toEqual({ width: 300, height: 200 });
    expect(fitWithin(0, 0, 1024)).toEqual({ width: 1, height: 1 });
  });

  it("takes a video's frames from the middles of equal parts", () => {
    expect(frameTimes(40, 4)).toEqual([5, 15, 25, 35]);
    expect(frameTimes(Number.NaN, 4)).toEqual([0]);
    expect(frameTimes(Infinity, 4)).toEqual([0]);
  });
});
