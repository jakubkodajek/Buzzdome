import { describe, expect, it } from "vitest";
import { addFiles, aiFileKind, decodedText, fitWithin, frameTimes } from "./ai-files";

const file = (name: string, size = 10, lastModified = 1) => new File([new Uint8Array(size)], name, { lastModified });

describe("the AI panel's files", () => {
  it("takes pictures, videos and text files by their names", () => {
    expect(["a.JPG", "b.png", "c.webp"].map(aiFileKind)).toEqual(["picture", "picture", "picture"]);
    expect(["d.mp4", "e.webm"].map(aiFileKind)).toEqual(["video", "video"]);
    expect(["f.txt", "g.MD", "h.csv"].map(aiFileKind)).toEqual(["text", "text", "text"]);
    expect(["i.pdf", "j.mp3", "k", "l.docx"].map(aiFileKind)).toEqual([null, null, null, null]);
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

  it("reads text files saved as UTF-8, UTF-16 or Czech Windows", () => {
    const czech = "Příliš žluťoučký kůň";
    expect(decodedText(new TextEncoder().encode(czech))).toBe(czech);
    // UTF-16 with its byte order mark, as Excel's Unicode text.
    const utf16 = new Uint8Array([0xff, 0xfe, ...[...czech].flatMap((c) => [c.charCodeAt(0) & 0xff, c.charCodeAt(0) >> 8])]);
    expect(decodedText(utf16)).toBe(czech);
    // Windows-1250: ř 0xF8, í 0xED, š 0x9A, ž 0x9E, ť 0x9D, č 0xE8, ý 0xFD, ů 0xF9, ň 0xF2.
    const cp1250 = new Uint8Array([0x50, 0xf8, 0xed, 0x6c, 0x69, 0x9a]);
    expect(decodedText(cp1250)).toBe("Příliš");
    // A UTF-8 character cut at the end of what was read is dropped, not read as Windows-1250.
    const cut = new TextEncoder().encode("kůň").slice(0, -1);
    expect(decodedText(cut)).toBe("ků");
  });
});
