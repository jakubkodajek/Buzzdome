import { describe, expect, it } from "vitest";
import { decibels, minutesAndSeconds, parseTrackTime, scoreDigits, signed, trackTime } from "./format";

describe("format", () => {
  it("writes scores", () => {
    expect([signed(10), signed(0), signed(-5)]).toEqual(["+10", "0", "-5"]);
    expect([scoreDigits(42), scoreDigits(-5), scoreDigits(1234)]).toEqual(["042", "-05", "1234"]);
  });

  it("writes a countdown's seconds as minutes and seconds", () => {
    expect([minutesAndSeconds(0), minutesAndSeconds(7), minutesAndSeconds(90), minutesAndSeconds(600)]).toEqual(["0:00", "0:07", "1:30", "10:00"]);
  });

  it("writes levels and changes in dB to a tenth", () => {
    expect([decibels(2.46), decibels(-18.34), decibels(0), decibels(-0.01)]).toEqual(["+2.5", "-18.3", "0.0", "0.0"]);
    expect(decibels(3, false)).toBe("3.0");
  });

  it("writes times in a track to a tenth of a second", () => {
    expect([trackTime(0), trackTime(75_500), trackTime(59_960), trackTime(3_723_000)]).toEqual(["0:00.0", "1:15.5", "1:00.0", "1:02:03.0"]);
  });

  it("reads times typed in any of the usual ways", () => {
    for (const text of ["75", "1:15", "0:01:15", " 1:15.0 ", "75,0"]) expect(parseTrackTime(text), text).toBe(75_000);
    expect(parseTrackTime("1:15.5")).toBe(75_500);
    expect(parseTrackTime(trackTime(3_723_400))).toBe(3_723_400);
    for (const text of ["", "abc", "1:2:3:4", "-5", "1.5:00", "1:", ":15"]) expect(parseTrackTime(text), text).toBeNull();
  });
});
