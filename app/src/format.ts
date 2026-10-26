/** Whole seconds left on a countdown, rounded up as clocks show them. */
export function clockSeconds(ms: number): string {
  return String(Math.ceil(ms / 1000));
}

/** Whole seconds as minutes and seconds, as a digital clock shows them: "0:07", "1:30". */
export function minutesAndSeconds(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/** A duration in seconds with `digits` decimals, e.g. "2.4". */
export function seconds(ms: number, digits = 1): string {
  return (ms / 1000).toFixed(digits);
}

/** A score change with its sign, e.g. "+10" or "-5". */
export function signed(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

/** A level or change in dB (or LUFS) to a tenth, with its sign when `sign`: "+2.5", "-18.3". */
export function decibels(value: number, sign = true): string {
  const text = value.toFixed(1);
  return sign && value > 0 ? `+${text}` : text === "-0.0" ? "0.0" : text;
}

/** A score padded to three digits for the score boards, e.g. "042". */
export function scoreDigits(n: number): string {
  return n < 0 ? `-${String(-n).padStart(2, "0")}` : String(n).padStart(3, "0");
}

/** A time in a track, to a tenth of a second: "1:15.5", or "1:02:03.0" past an hour. */
export function trackTime(ms: number): string {
  const tenths = Math.max(0, Math.round(ms / 100));
  const whole = Math.floor(tenths / 10);
  const [h, m, s] = [Math.floor(whole / 3600), Math.floor(whole / 60) % 60, whole % 60];
  const rest = `${String(s).padStart(2, "0")}.${tenths % 10}`;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
}

/**
 * A time typed into a field, in ms: seconds ("75"), minutes and seconds
 * ("1:15") or hours too ("0:01:15"), the seconds with decimals after a dot
 * or comma ("1:15.5", "1:15,5"). Null when the text is none of these.
 */
export function parseTrackTime(text: string): number | null {
  const parts = text.trim().replace(",", ".").split(":");
  const valid = parts.length <= 3 && parts.every((part, i) => (i === parts.length - 1 ? /^\d+(\.\d+)?$/ : /^\d+$/).test(part));
  return valid ? Math.round(parts.reduce((total, part) => total * 60 + Number(part), 0) * 1000) : null;
}

/** The file name at the end of `path`, whether it is separated by "/" or "\\". */
export function fileName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1);
}
