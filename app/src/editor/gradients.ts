// The gradients the gradient picker offers: ready-made ones and the user's
// own (up to five, kept per computer).

import { readPreference, writePreference } from "../preferences";
import { createStore } from "../store";
import { isColor } from "../theme/color";
import type { Gradient, GradientKind } from "../types";

export const GRADIENT_KINDS: readonly GradientKind[] = ["vertical", "horizontal", "diagonal", "diagonalUp", "radial"];

export const GRADIENT_PRESETS: readonly Gradient[] = [
  { from: "#1E3C72", to: "#2A5298", kind: "vertical" },
  { from: "#0F2027", to: "#2C5364", kind: "vertical" },
  { from: "#141E30", to: "#243B55", kind: "diagonal" },
  { from: "#000000", to: "#434343", kind: "vertical" },
  { from: "#3A1C71", to: "#D76D77", kind: "diagonal" },
  { from: "#42275A", to: "#734B6D", kind: "vertical" },
  { from: "#8E0E00", to: "#1F1C18", kind: "vertical" },
  { from: "#CB2D3E", to: "#EF473A", kind: "horizontal" },
  { from: "#F7971E", to: "#FFD200", kind: "diagonal" },
  { from: "#FF512F", to: "#F09819", kind: "horizontal" },
  { from: "#11998E", to: "#38EF7D", kind: "diagonal" },
  { from: "#134E5E", to: "#71B280", kind: "vertical" },
  { from: "#00B4DB", to: "#0083B0", kind: "vertical" },
  { from: "#2193B0", to: "#6DD5ED", kind: "diagonalUp" },
  { from: "#4568DC", to: "#B06AB3", kind: "horizontal" },
  { from: "#FFFFFF", to: "#D7DDE8", kind: "vertical" },
  { from: "#2C3E50", to: "#000000", kind: "radial" },
  { from: "#FDC830", to: "#F37335", kind: "radial" },
];

export const MY_GRADIENTS_LIMIT = 5;

const MY_GRADIENTS_KEY = "quiz-show.gradients.mine";

export function isGradient(value: unknown): value is Gradient {
  if (typeof value !== "object" || value === null) return false;
  const { from, to, kind } = value as Record<string, unknown>;
  return typeof from === "string" && isColor(from) && typeof to === "string" && isColor(to) && GRADIENT_KINDS.includes(kind as GradientKind);
}

/** A gradient written as JSON (as Copy value writes it); null for other text. */
export function readGradient(text: string): Gradient | null {
  try {
    const value: unknown = JSON.parse(text);
    return isGradient(value) ? { from: value.from, to: value.to, kind: value.kind } : null;
  } catch {
    return null;
  }
}

export function sameGradient(a: Gradient, b: Gradient): boolean {
  return a.from === b.from && a.to === b.to && a.kind === b.kind;
}

/** The user's own gradients, the latest first; what cannot be read is left out. */
export const myGradients = createStore<readonly Gradient[]>(readMine());

function readMine(): readonly Gradient[] {
  const stored = readPreference(MY_GRADIENTS_KEY);
  try {
    const value: unknown = stored === null ? [] : JSON.parse(stored);
    return Array.isArray(value) ? value.filter(isGradient).slice(0, MY_GRADIENTS_LIMIT) : [];
  } catch {
    return [];
  }
}

function setMine(gradients: readonly Gradient[]): void {
  myGradients.set(gradients);
  writePreference(MY_GRADIENTS_KEY, JSON.stringify(gradients));
}

/** Adds `gradient` first among the user's own; the oldest goes beyond five. */
export function addMyGradient(gradient: Gradient): void {
  setMine(withMine(myGradients.get(), gradient));
}

export function removeMyGradient(gradient: Gradient): void {
  setMine(myGradients.get().filter((g) => !sameGradient(g, gradient)));
}

export function withMine(mine: readonly Gradient[], gradient: Gradient): readonly Gradient[] {
  return [gradient, ...mine.filter((g) => !sameGradient(g, gradient))].slice(0, MY_GRADIENTS_LIMIT);
}
