import type { Cue } from "../types";
import { audioContext } from "./context";

// Placeholder tones until the show has its own sound set.
const TONES: Record<Cue, { freq: number[]; ms: number }> = {
  clockStart: { freq: [660], ms: 150 },
  buzz: { freq: [880, 1320], ms: 180 },
  correct: { freq: [523, 659, 784], ms: 160 },
  wrong: { freq: [220, 160], ms: 260 },
  timeUp: { freq: [440, 330], ms: 300 },
  reveal: { freq: [587, 784], ms: 160 },
};

export function playCue(cue: Cue) {
  const ctx = audioContext();
  const { freq, ms } = TONES[cue];
  freq.forEach((f, i) => {
    const start = ctx.currentTime + (i * ms) / 1000;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = f;
    osc.type = "triangle";
    gain.gain.setValueAtTime(0.25, start);
    gain.gain.exponentialRampToValueAtTime(0.001, start + ms / 1000);
    osc.connect(gain).connect(ctx.destination);
    osc.start(start);
    osc.stop(start + ms / 1000);
  });
}
