/** Whole seconds left on a countdown, rounded up as clocks show them. */
export function clockSeconds(ms: number): string {
  return String(Math.ceil(ms / 1000));
}

/** A duration in seconds with `digits` decimals, e.g. "2.4". */
export function seconds(ms: number, digits = 1): string {
  return (ms / 1000).toFixed(digits);
}

/** A score change with its sign, e.g. "+10" or "-5". */
export function signed(n: number): string {
  return n > 0 ? `+${n}` : String(n);
}

/** A score padded to three digits for the score boards, e.g. "042". */
export function scoreDigits(n: number): string {
  return n < 0 ? `-${String(-n).padStart(2, "0")}` : String(n).padStart(3, "0");
}
