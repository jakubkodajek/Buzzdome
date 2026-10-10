// Colours as a quiz file writes them (`#RRGGBB`, or `#RRGGBBAA` with an
// opacity; quiz_core::style::Color), and what the colour picker needs:
// reading a typed or pasted colour, CSS with a transparency, and hue,
// saturation and value.

import type { Color } from "../types";

/** Red, green and blue 0 to 255; alpha 0 (invisible) to 1 (solid). */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

/** Hue 0 to 360; saturation and value 0 to 1. */
export interface Hsv {
  h: number;
  s: number;
  v: number;
}

const HEX_COLOR = /^#([0-9a-f]{6}|[0-9a-f]{8})$/i;

/** Whether `text` is a colour as a quiz file writes it. */
export function isColor(text: string): text is Color {
  return HEX_COLOR.test(text);
}

/** The channels of `color`; null when it is not `#RRGGBB` or `#RRGGBBAA`. */
export function parseColor(color: string): Rgba | null {
  if (!isColor(color)) return null;
  const channel = (at: number) => parseInt(color.slice(at, at + 2), 16);
  return { r: channel(1), g: channel(3), b: channel(5), a: color.length === 9 ? channel(7) / 255 : 1 };
}

/** As a quiz file writes it, in upper case: `#RRGGBB`, with the opacity only when it is not solid. */
export function formatColor({ r, g, b, a }: Rgba): Color {
  const hex = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, "0");
  const alpha = Math.round(a * 255) >= 255 ? "" : hex(a * 255);
  return `#${hex(r)}${hex(g)}${hex(b)}${alpha}`.toUpperCase();
}

/**
 * A colour typed or pasted from another program: with or without `#`, as
 * 3, 4, 6 or 8 hex digits (`#1E4A6B`, `1e4a6b`, `#fff`), spaces around
 * ignored. Null when the text is not one.
 */
export function readColor(text: string): Color | null {
  const digits = text.trim().replace(/^#/, "");
  if (!/^([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(digits)) return null;
  const full = digits.length <= 4 ? [...digits].map((d) => d + d).join("") : digits;
  const parsed = parseColor(`#${full}`);
  return parsed && formatColor(parsed);
}

/** How much of `color` covers what is behind it, 0 to 1, with `transparency` percent of it showing through on top of its own opacity. */
export function opacity(color: Color, transparency = 0): number {
  return (parseColor(color)?.a ?? 0) * (1 - transparency / 100);
}

/** `color` for CSS, with `transparency` percent of it showing through on top of its own opacity. */
export function cssColor(color: Color, transparency = 0): string {
  const parsed = parseColor(color);
  if (!parsed) return "transparent";
  return `rgba(${parsed.r}, ${parsed.g}, ${parsed.b}, ${Math.round(opacity(color, transparency) * 1000) / 1000})`;
}

export function toHsv({ r, g, b }: Pick<Rgba, "r" | "g" | "b">): Hsv {
  const [red, green, blue] = [r / 255, g / 255, b / 255];
  const max = Math.max(red, green, blue);
  const range = max - Math.min(red, green, blue);
  let h = 0;
  if (range > 0) {
    if (max === red) h = ((green - blue) / range) % 6;
    else if (max === green) h = (blue - red) / range + 2;
    else h = (red - green) / range + 4;
  }
  return { h: (h * 60 + 360) % 360, s: max === 0 ? 0 : range / max, v: max };
}

export function fromHsv({ h, s, v }: Hsv, a = 1): Rgba {
  const channel = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255);
  };
  return { r: channel(5), g: channel(3), b: channel(1), a };
}
