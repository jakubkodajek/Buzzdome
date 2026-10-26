#!/usr/bin/env python3
"""Makes the app's own sound effects and music (spec SN-1, SN-2).

Every sound is synthesised here from sine waves and seeded noise, so the
library holds no recording and no third-party material, and the files come
out the same each run (up to the MP3 encoder's version). Run it from any
folder after changing a sound:

    python3 app/src/audio/library/make_sounds.py

It writes `<id>.mp3` next to itself for each sound in `EFFECTS` and
`MUSIC`, and `sounds.json` with each sound's kind and length, and for music
the length of its loop: the file runs on past it with what still rings,
which the app plays over the start of the next round (`show-audio.ts`).
Needs NumPy and an `ffmpeg` with libmp3lame on the PATH.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import wave
import zlib
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from numpy.typing import NDArray

Signal = NDArray[np.float64]

SR = 48_000
HERE = Path(__file__).resolve().parent
# Every sound is brought to this loudness (ITU-R BS.1770-4, gated), as the
# app's Normalize does at its Standard target (spec SN-9), unless its peak
# would pass PEAK_DB first.
TARGET_LUFS = -16.0
PEAK_DB = -1.0
# LAME variable bit rate, about 130 kbit/s for stereo music.
MP3_QUALITY = "5"


# --- Building blocks -------------------------------------------------------


def samples(seconds: float) -> int:
    return int(round(seconds * SR))


def times(seconds: float) -> Signal:
    return np.arange(samples(seconds)) / SR


def note(name: str) -> float:
    """The frequency of a note such as `A4` or `F#3` (equal temperament, A4 = 440 Hz)."""
    steps = {"C": -9, "D": -7, "E": -5, "F": -4, "G": -2, "A": 0, "B": 2}
    letter, rest = name[0], name[1:]
    shift = 0
    while rest and rest[0] in "#b":
        shift += 1 if rest[0] == "#" else -1
        rest = rest[1:]
    return 440.0 * 2 ** ((steps[letter] + shift + 12 * (int(rest) - 4)) / 12)


def cents(frequency: float, amount: float) -> float:
    return frequency * 2 ** (amount / 1200)


def phase(frequency: float | Signal, n: int) -> Signal:
    if np.isscalar(frequency):
        return 2 * np.pi * float(frequency) * np.arange(n) / SR
    return 2 * np.pi * np.cumsum(frequency) / SR


def additive(frequency: float | Signal, seconds: float, amplitude: Callable[[int], float | Signal], top: int = 40) -> Signal:
    """Harmonics 1.. of `frequency`, each at `amplitude(k)`, none above 0.45 SR."""
    n = samples(seconds)
    ph = phase(frequency, n)
    highest = float(np.max(frequency))
    out = np.zeros(n)
    for k in range(1, top + 1):
        if k * highest > 0.45 * SR:
            break
        out += amplitude(k) * np.sin(k * ph)
    return out


def saw(frequency: float | Signal, seconds: float, top: int = 40) -> Signal:
    return additive(frequency, seconds, lambda k: 1 / k, top)


def square(frequency: float | Signal, seconds: float, top: int = 40) -> Signal:
    return additive(frequency, seconds, lambda k: (k % 2) / k, top)


def sine(frequency: float | Signal, seconds: float) -> Signal:
    return np.sin(phase(frequency, samples(seconds)))


def noise(seconds: float, rng: np.random.Generator) -> Signal:
    return rng.uniform(-1, 1, samples(seconds))


def ending(n: int, seconds: float = 0.02) -> Signal:
    """1, then down to 0 over the last `seconds`, so a sound cut short ends without a click."""
    out = np.ones(n)
    fade = min(n, samples(seconds))
    out[n - fade :] = 0.5 + 0.5 * np.cos(np.linspace(0, np.pi, fade))
    return out


def decay(seconds: float, tau: float, attack: float = 0.002) -> Signal:
    """An envelope that rises over `attack`, falls by e every `tau` seconds, and ends at 0."""
    t = times(seconds)
    return np.minimum(1, t / max(attack, 1e-6)) * np.exp(-np.maximum(0, t - attack) / tau) * ending(len(t))


def adsr(seconds: float, attack: float, decay_s: float, sustain: float, release: float) -> Signal:
    """A held note of `seconds` with its release after it, so `seconds + release` long."""
    t = times(seconds + release)
    rise = np.minimum(1, t / max(attack, 1e-6))
    fall = sustain + (1 - sustain) * np.exp(-np.maximum(0, t - attack) / max(decay_s, 1e-6))
    held = rise * fall
    end = held[min(len(held) - 1, samples(seconds))]
    after = t >= seconds
    held[after] = end * np.maximum(0, 1 - (t[after] - seconds) / max(release, 1e-6))
    return held


def fit(signal: Signal, n: int) -> Signal:
    return signal[:n] if len(signal) >= n else np.pad(signal, (0, n - len(signal)))


def spectral(signal: Signal, gain: Callable[[Signal], Signal]) -> Signal:
    """Filters with the magnitude `gain(f)` and no phase shift (filter noise, then shape it)."""
    size = 1 << int(np.ceil(np.log2(len(signal) + SR // 10)))
    spectrum = np.fft.rfft(signal, size)
    f = np.fft.rfftfreq(size, 1 / SR)
    return np.fft.irfft(spectrum * gain(f), size)[: len(signal)]


def lowpass(signal: Signal, cutoff: float, order: int = 2) -> Signal:
    return spectral(signal, lambda f: 1 / np.sqrt(1 + (f / cutoff) ** (2 * order)))


def highpass(signal: Signal, cutoff: float, order: int = 2) -> Signal:
    return spectral(signal, lambda f: 1 / np.sqrt(1 + (cutoff / np.maximum(f, 1e-3)) ** (2 * order)))


def bandpass(signal: Signal, low: float, high: float, order: int = 2) -> Signal:
    return highpass(lowpass(signal, high, order), low, order)


def sweep(signal: Signal, centre: Callable[[Signal], Signal], width: float = 1.0) -> Signal:
    """A band-pass whose centre follows `centre(t)`, `width` octaves wide (short-time FFT)."""
    frame, hop = 2048, 512
    window = np.hanning(frame)
    padded = np.pad(signal, (frame, frame))
    out = np.zeros(len(padded))
    norm = np.zeros(len(padded))
    f = np.fft.rfftfreq(frame, 1 / SR)
    for start in range(0, len(padded) - frame, hop):
        middle = (start + frame / 2 - frame) / SR
        c = float(centre(np.array([max(0.0, middle)]))[0])
        octaves = np.log2(np.maximum(f, 1.0) / c) / (width / 2)
        gain = 1 / np.sqrt(1 + octaves**4)
        out[start : start + frame] += np.fft.irfft(np.fft.rfft(padded[start : start + frame] * window) * gain, frame) * window
        norm[start : start + frame] += window**2
    return (out / np.maximum(norm, 1e-3))[frame:-frame]


def saturate(signal: Signal, drive: float) -> Signal:
    return np.tanh(drive * signal) / np.tanh(drive)


# --- Stereo and space ------------------------------------------------------


def stereo(signal: Signal, pan: float = 0.0) -> Signal:
    """Mono to stereo, `pan` from -1 (left) to 1 (right), at equal power."""
    angle = (pan + 1) * np.pi / 4
    return np.vstack([np.cos(angle) * signal, np.sin(angle) * signal]) * np.sqrt(2)


def as_stereo(signal: Signal) -> Signal:
    return signal if signal.ndim == 2 else stereo(signal)


def reverb(signal: Signal, seconds: float, wet: float, rng: np.random.Generator, brightness: float = 6000.0) -> Signal:
    """A room `seconds` long (to -60 dB): seeded noise that darkens as it dies, one per side."""
    signal = as_stereo(signal)
    n = samples(seconds)
    t = np.arange(n) / SR
    envelope = np.exp(-6.9 * t / seconds)
    out = np.zeros((2, signal.shape[1] + n))
    for side in range(2):
        source = rng.standard_normal(n)
        dark = lowpass(source, brightness / 4)
        bright = lowpass(source, brightness)
        mix = np.exp(-t / (seconds / 4))
        ir = envelope * (mix * bright + (1 - mix) * dark)
        ir[: samples(0.012)] = 0
        ir /= np.sqrt(np.sum(ir**2))
        size = 1 << int(np.ceil(np.log2(signal.shape[1] + n)))
        wet_side = np.fft.irfft(np.fft.rfft(signal[side], size) * np.fft.rfft(ir, size), size)
        out[side] = wet * wet_side[: out.shape[1]]
        out[side, : signal.shape[1]] += signal[side]
    return out


def echo(signal: Signal, delay: float, feedback: float, repeats: int = 4) -> Signal:
    """Ping-pong echoes, each `feedback` times the one before."""
    signal = as_stereo(signal)
    step = samples(delay)
    out = np.zeros((2, signal.shape[1] + step * repeats))
    out[:, : signal.shape[1]] += signal
    for i in range(1, repeats + 1):
        side = i % 2
        out[side, step * i : step * i + signal.shape[1]] += feedback**i * signal.mean(axis=0)
    return out


class Mix:
    """A stereo buffer that sounds are placed into."""

    def __init__(self, seconds: float) -> None:
        self.buffer = np.zeros((2, samples(seconds)))

    def add(self, sound: Signal, at: float, gain: float = 1.0, pan: float = 0.0) -> None:
        part = stereo(sound, pan) if sound.ndim == 1 else sound
        start = samples(at)
        end = min(self.buffer.shape[1], start + part.shape[1])
        if end > start:
            self.buffer[:, start:end] += gain * part[:, : end - start]


# --- Instruments -----------------------------------------------------------


def kick(rng: np.random.Generator, punch: float = 1.0) -> Signal:
    t = times(0.5)
    body = np.sin(phase(45 + 110 * np.exp(-t / 0.035), len(t))) * np.exp(-t / 0.2)
    click = highpass(noise(0.5, rng), 2500) * np.exp(-t / 0.004) * 0.35 * punch
    return saturate(body + click, 1.6) * ending(len(t))


def snare(rng: np.random.Generator, tone: float = 1.0) -> Signal:
    t = times(0.4)
    drum = (np.sin(2 * np.pi * 190 * t) + 0.6 * np.sin(2 * np.pi * 330 * t)) * np.exp(-t / 0.05) * 0.5 * tone
    rattle = bandpass(noise(0.4, rng), 1200, 9000) * np.exp(-t / 0.11)
    return (drum + rattle) * ending(len(t))


def clap(rng: np.random.Generator) -> Signal:
    t = times(0.35)
    source = bandpass(noise(0.35, rng), 900, 3200)
    envelope = np.zeros(len(t))
    for at in (0.0, 0.011, 0.021, 0.033):
        envelope += (t >= at) * np.exp(-np.maximum(0, t - at) / 0.007)
    envelope += (t >= 0.033) * np.exp(-np.maximum(0, t - 0.033) / 0.1) * 0.6
    return source * envelope * ending(len(t))


def hat(rng: np.random.Generator, open_: bool = False) -> Signal:
    seconds = 0.5 if open_ else 0.12
    source = highpass(noise(seconds, rng), 7000, 3)
    return source * decay(seconds, 0.22 if open_ else 0.03, 0.001)


def shaker(rng: np.random.Generator) -> Signal:
    return bandpass(noise(0.15, rng), 4000, 12000) * decay(0.15, 0.04, 0.02)


def tom(frequency: float, rng: np.random.Generator, seconds: float = 0.9) -> Signal:
    t = times(seconds)
    body = np.sin(phase(frequency * (1 + 0.25 * np.exp(-t / 0.04)), len(t))) * np.exp(-t / (seconds / 3))
    skin = bandpass(noise(seconds, rng), 200, 2000) * np.exp(-t / 0.03) * 0.3
    return (body + skin) * ending(len(t))


def timpani(frequency: float, rng: np.random.Generator, seconds: float = 2.0) -> Signal:
    t = times(seconds)
    ratios = (1.0, 1.5, 1.98, 2.44)
    body = sum(np.sin(2 * np.pi * frequency * r * t) * np.exp(-t / (seconds / (2 + i))) / (1 + i) for i, r in enumerate(ratios))
    mallet = lowpass(noise(seconds, rng), 1500) * np.exp(-t / 0.02) * 0.5
    return (body + mallet) * ending(len(t))


def crash(rng: np.random.Generator, seconds: float = 2.5) -> Signal:
    t = times(seconds)
    metal = sum(np.sin(2 * np.pi * f * t + rng.uniform(0, 6.28)) for f in rng.uniform(3000, 11000, 40)) / 20
    hiss = highpass(noise(seconds, rng), 4500, 2)
    return (0.5 * metal + hiss) * decay(seconds, seconds / 4, 0.003)


def woodblock(frequency: float, rng: np.random.Generator) -> Signal:
    t = times(0.25)
    body = (np.sin(2 * np.pi * frequency * t) + 0.4 * np.sin(2 * np.pi * frequency * 2.3 * t)) * np.exp(-t / 0.025)
    knock = bandpass(noise(0.25, rng), 1500, 6000) * np.exp(-t / 0.003) * 0.4
    return (body + knock) * ending(len(t))


def bass(frequency: float, seconds: float, brightness: float = 900.0) -> Signal:
    total = seconds + 0.06
    t = times(total)
    cutoff = brightness * (0.6 + 1.4 * np.exp(-t / 0.08))
    tone = additive(frequency, total, lambda k: (1 / k) / np.sqrt(1 + (k * frequency / cutoff) ** 4), 24)
    sub = 0.6 * sine(frequency, total)
    return (tone + sub) * adsr(seconds, 0.004, 0.2, 0.7, 0.06)


def upright(frequency: float, seconds: float) -> Signal:
    total = seconds + 0.1
    t = times(total)
    tone = additive(frequency, total, lambda k: np.exp(-t * (1.5 + 4 * k)) / k**1.5, 12)
    return tone * adsr(seconds, 0.008, 0.4, 0.3, 0.1)


def epiano(frequency: float, seconds: float, velocity: float = 0.8) -> Signal:
    total = seconds + 0.3
    t = times(total)
    index = 1.6 * velocity * np.exp(-t / 0.4) + 0.25
    tone = np.sin(2 * np.pi * frequency * t + index * np.sin(2 * np.pi * frequency * t))
    if 14 * frequency < 0.45 * SR:
        tone += 0.08 * velocity * np.sin(2 * np.pi * 14 * frequency * t) * np.exp(-t / 0.015)
    envelope = np.exp(-t / 1.6) * adsr(seconds, 0.003, 10, 1, 0.3)
    tremolo = 1 + 0.08 * np.sin(2 * np.pi * 4.5 * t)
    return tone * envelope * tremolo * velocity


def pad(frequencies: Sequence[float], seconds: float, brightness: float = 1600.0) -> Signal:
    total = seconds + 0.9
    voices = np.zeros((2, samples(total)))
    for f in frequencies:
        for detune, pan in ((-9, -0.6), (0, 0), (9, 0.6)):
            voices += stereo(saw(cents(f, detune), total, 16), pan)
    voices[0] = lowpass(voices[0], brightness)
    voices[1] = lowpass(voices[1], brightness)
    return voices * adsr(seconds, 0.45, 1, 1, 0.9) / (3 * len(frequencies))


def pluck(frequency: float, seconds: float, brightness: float = 1.0) -> Signal:
    total = seconds + 0.12
    t = times(total)
    tone = additive(frequency, total, lambda k: np.exp(-t * (2 + 6 * k / brightness)) / k, 20)
    return tone * adsr(seconds, 0.002, 10, 1, 0.12)


def lead(frequency: float, seconds: float) -> Signal:
    total = seconds + 0.12
    t = times(total)
    wobble = frequency * (1 + 0.004 * np.sin(2 * np.pi * 5.5 * t) * np.minimum(1, t / 0.3))
    tone = additive(wobble, total, lambda k: (1 / k) * (0.55 if k % 2 == 0 else 1.0) / np.sqrt(1 + (k * frequency / 3500) ** 4), 24)
    return tone * adsr(seconds, 0.01, 0.15, 0.75, 0.12)


def brass(frequency: float, seconds: float, velocity: float = 0.9) -> Signal:
    total = seconds + 0.12
    t = times(total)
    swell = np.minimum(1, t / 0.06) * (1 + 0.25 * np.exp(-np.maximum(0, t - 0.06) / 0.08)) * 0.8
    bright = 1 + 7 * velocity * swell
    vibrato = 1 + 0.004 * np.sin(2 * np.pi * 5.2 * t) * np.clip((t - 0.25) / 0.3, 0, 1)
    voices = sum(
        additive(cents(frequency, d) * vibrato, total, lambda k: np.exp(-(k - 1) / bright) / k**0.5, 30) for d in (-5, 5)
    )
    return voices * adsr(seconds, 0.03, 0.3, 0.85, 0.12) * velocity / 2


def bell(frequency: float, seconds: float, partials: Sequence[tuple[float, float, float]] | None = None) -> Signal:
    """Inharmonic partials (ratio, level, decay in seconds)."""
    partials = partials or ((1, 1, 1.0), (2.76, 0.5, 0.45), (5.4, 0.25, 0.22), (8.93, 0.12, 0.12))
    t = times(seconds)
    out = np.zeros(len(t))
    for ratio, level, tau in partials:
        if frequency * ratio < 0.45 * SR:
            out += level * np.sin(2 * np.pi * frequency * ratio * t) * np.exp(-t / tau)
    return out * np.minimum(1, t / 0.001) * ending(len(t), 0.08)


def gong(frequency: float, seconds: float, rng: np.random.Generator) -> Signal:
    t = times(seconds)
    ratios = (1.0, 1.52, 2.03, 2.66, 3.21, 4.1, 5.43, 6.8)
    out = np.zeros(len(t))
    for i, r in enumerate(ratios):
        beat = 1 + 0.003 * np.sin(2 * np.pi * (0.7 + i * 0.3) * t)
        out += np.sin(phase(frequency * r * beat, len(t))) * np.exp(-t / (seconds / (3 + i * 0.5))) / (1 + 0.6 * i)
    strike = lowpass(noise(seconds, rng), 3000) * np.exp(-t / 0.04) * 0.4
    swell = 0.6 + 0.4 * np.minimum(1, t / 0.25)
    return (out * swell + strike) * ending(len(t), 0.2)


# --- Music -----------------------------------------------------------------


@dataclass
class Song:
    """A loop of `bars` bars at `bpm`, with `tail` seconds after it for what still rings."""

    bpm: float
    bars: int
    tail: float = 3.0

    @property
    def beat(self) -> float:
        return 60 / self.bpm

    @property
    def loop(self) -> float:
        return self.bars * 4 * self.beat

    def at(self, bar: int, beat: float = 0.0) -> float:
        return (bar * 4 + beat) * self.beat


def chord(names: str) -> list[float]:
    return [note(name) for name in names.split()]


def lobby(rng: np.random.Generator) -> tuple[Signal, float]:
    """House-pop in F at 118 BPM: drums, bass, electric piano, a lead from bar 9."""
    song = Song(118, 16)
    drums, keys, low, top = (Mix(song.loop + song.tail) for _ in range(4))
    progression = ["F3 A3 C4 E4 G4", "C3 G3 C4 E4 D4", "D3 A3 C4 F4", "A#2 A#3 D4 F4 A4"]
    roots = ["F2", "C2", "D2", "A#1"]
    k, cl, ch, oh, sh = kick(rng), clap(rng), hat(rng), hat(rng, True), shaker(rng)
    for bar in range(song.bars):
        for beat in range(4):
            drums.add(k, song.at(bar, beat), 0.9)
            drums.add(ch, song.at(bar, beat + 0.5), 0.35, 0.3)
            for sixteenth in range(4):
                drums.add(sh, song.at(bar, beat + sixteenth / 4), 0.12 + 0.06 * (sixteenth == 2), -0.4)
        drums.add(cl, song.at(bar, 1), 0.5)
        drums.add(cl, song.at(bar, 3), 0.5)
        if bar % 4 == 3:
            drums.add(oh, song.at(bar, 3.5), 0.3, 0.3)
        index = bar % 4
        root = note(roots[index])
        for eighth in (0.5, 1.5, 2.5, 3.5):
            low.add(bass(root if eighth != 2.5 else root * 2, song.beat * 0.4), song.at(bar, eighth), 0.55)
        for beat, length in ((0, 1.0), (1.5, 0.5), (2.5, 1.2)):
            voicing = sum(epiano(f, song.beat * length, 0.7) for f in chord(progression[index])[1:])
            keys.add(voicing, song.at(bar, beat), 0.16, 0.15)
    melody = [
        [("A4", 0, 1), ("C5", 1, 0.5), ("D5", 1.5, 0.5), ("C5", 2, 1), ("A4", 3, 1)],
        [("G4", 0, 1), ("A4", 1, 0.5), ("G4", 1.5, 0.5), ("E4", 2, 2)],
        [("F4", 0, 0.5), ("A4", 0.5, 0.5), ("D5", 1, 1), ("C5", 2, 0.5), ("A4", 2.5, 0.5), ("G4", 3, 1)],
        [("F4", 0, 3)],
    ]
    ending = [("A4", 0, 1), ("G4", 1, 1), ("F4", 2, 2)]
    for bar in range(8, 16):
        phrase = ending if bar == 15 else melody[(bar - 8) % 4]
        for name, beat, length in phrase:
            top.add(lead(note(name), song.beat * length * 0.9), song.at(bar, beat), 0.2)
    music = drums.buffer + low.buffer + reverb(keys.buffer, 1.6, 0.35, rng)[:, : drums.buffer.shape[1]]
    music += echo(reverb(top.buffer, 1.8, 0.3, rng), song.beat * 0.75, 0.35)[:, : drums.buffer.shape[1]]
    return music, song.loop


def lounge(rng: np.random.Generator) -> tuple[Signal, float]:
    """Calm jazz in G at 88 BPM: brushes, walking bass, electric piano."""
    song = Song(88, 16)
    drums, keys, low = (Mix(song.loop + song.tail) for _ in range(3))
    changes = ["A2 C4 E4 G4 B4", "D3 C4 F#4 A4 E5", "G2 B3 D4 F#4 A4", "E3 D4 G4 B4"]
    walks = [["A2", "C3", "E3", "G3"], ["D3", "F#3", "A3", "C3"], ["G2", "B2", "D3", "F#3"], ["E3", "G3", "B2", "D3"]]
    k, sn, ch = kick(rng, 0.4), snare(rng, 0.3), hat(rng)
    for bar in range(song.bars):
        index = bar % 4
        drums.add(k, song.at(bar, 0), 0.25)
        drums.add(k, song.at(bar, 2.66), 0.16)
        for beat in (1, 3):
            drums.add(lowpass(sn, 5000), song.at(bar, beat), 0.18, 0.2)
        for beat in range(4):
            drums.add(ch, song.at(bar, beat), 0.16, -0.3)
            drums.add(ch, song.at(bar, beat + 0.66), 0.1, -0.3)
        for beat, name in enumerate(walks[index]):
            low.add(upright(note(name), song.beat * 0.9), song.at(bar, beat), 0.7)
        hits = ((0, 1.4), (2.66, 1.2)) if bar % 2 == 0 else ((0.66, 1.0), (2, 1.8))
        for beat, length in hits:
            voicing = sum(epiano(f, song.beat * length, 0.6) for f in chord(changes[index])[1:])
            keys.add(voicing, song.at(bar, beat), 0.15, -0.1)
    music = drums.buffer + low.buffer + reverb(keys.buffer, 2.0, 0.4, rng)[:, : drums.buffer.shape[1]]
    return music, song.loop


def thinking(rng: np.random.Generator) -> tuple[Signal, float]:
    """Thinking time in D minor at 100 BPM: a ticking clock, a pulse, slow chords."""
    song = Song(100, 16)
    drums, pads, low, bells = (Mix(song.loop + song.tail) for _ in range(4))
    chords = ["D3 F3 A3 D4", "A#2 D3 F3 A#3", "G2 A#2 D3 G3", "A2 C#3 E3 A3"]
    roots = ["D2", "A#1", "G1", "A1"]
    tick, tock, k = woodblock(1800, rng), woodblock(1350, rng), kick(rng, 0.3)
    for bar in range(song.bars):
        index = (bar // 2) % 4
        for beat in range(4):
            drums.add(tick if beat % 2 == 0 else tock, song.at(bar, beat), 0.22, 0.35)
            drums.add(hat(rng), song.at(bar, beat + 0.5), 0.07, -0.35)
        drums.add(k, song.at(bar, 0), 0.5)
        drums.add(k, song.at(bar, 2), 0.35)
        for eighth in range(8):
            low.add(bass(note(roots[index]), song.beat * 0.35, 500), song.at(bar, eighth / 2), 0.45 if eighth % 2 == 0 else 0.3)
        if bar % 2 == 0:
            pads.add(pad(chord(chords[index]), song.beat * 8 - 0.3, 1100), song.at(bar), 0.9)
        if bar % 4 == 3:
            for i, name in enumerate(("A5", "F5", "D5")):
                bells.add(bell(note(name), 2.5), song.at(bar, 2 + i * 0.5), 0.1, 0.4 - 0.4 * i)
    music = drums.buffer + low.buffer + reverb(pads.buffer, 2.5, 0.35, rng)[:, : drums.buffer.shape[1]]
    music += reverb(bells.buffer, 3.0, 0.6, rng)[:, : drums.buffer.shape[1]]
    return music, song.loop


def race(rng: np.random.Generator) -> tuple[Signal, float]:
    """Against the clock in A minor at 128 BPM: plucked sixteenths over a driving beat."""
    song = Song(128, 16)
    drums, plucks, low = (Mix(song.loop + song.tail) for _ in range(3))
    figures = [["A3", "C4", "E4", "A4"], ["F3", "A3", "C4", "F4"], ["C3", "E3", "G3", "C4"], ["G3", "B3", "D4", "G4"]]
    roots = ["A1", "F1", "C2", "G1"]
    k, sn, ch = kick(rng), snare(rng), hat(rng)
    for bar in range(song.bars):
        index = bar % 4
        for beat in range(4):
            drums.add(k, song.at(bar, beat), 0.8)
            drums.add(ch, song.at(bar, beat + 0.5), 0.3, 0.3)
            drums.add(ch, song.at(bar, beat + 0.75), 0.12, 0.3)
        drums.add(sn, song.at(bar, 1), 0.45)
        drums.add(sn, song.at(bar, 3), 0.45)
        if bar % 4 == 3:
            for i in range(4):
                drums.add(sn, song.at(bar, 3 + i / 4), 0.2 + 0.08 * i)
        for sixteenth in range(16):
            name = figures[index][sixteenth % 4]
            plucks.add(pluck(note(name), song.beat * 0.2, 0.8), song.at(bar, sixteenth / 4), 0.22, 0.5 if sixteenth % 2 else -0.5)
        for eighth in range(8):
            low.add(bass(note(roots[index]), song.beat * 0.4, 700), song.at(bar, eighth / 2), 0.5)
    music = drums.buffer + low.buffer + reverb(plucks.buffer, 1.2, 0.25, rng)[:, : drums.buffer.shape[1]]
    return music, song.loop


def drumroll(rng: np.random.Generator) -> tuple[Signal, float]:
    """A snare roll over rolling timpani and a held chord, for the build-up to the winner."""
    song = Song(100, 4, tail=2.5)
    drums, strings = (Mix(song.loop + song.tail) for _ in range(2))
    hit = snare(rng, 0.2)
    steps = int(song.loop * 15)
    for i in range(steps):
        drums.add(hit, i / 15, 0.13 + 0.05 * rng.random(), 0.15 * np.sin(i))
    for i in range(int(song.loop * 6)):
        drums.add(timpani(note("D2") if i % 2 == 0 else note("A1"), rng, 0.8), i / 6, 0.22)
    strings.add(pad(chord("D3 A3 E4 F4"), song.loop - 0.4, 900), 0.0, 0.8)
    music = reverb(drums.buffer, 1.4, 0.3, rng)[:, : drums.buffer.shape[1]] + reverb(strings.buffer, 2.0, 0.4, rng)[:, : drums.buffer.shape[1]]
    return music, song.loop


def victory(rng: np.random.Generator) -> tuple[Signal, float]:
    """A winner's march in C at 120 BPM: brass chords and a fanfare tune over drums."""
    song = Song(120, 16)
    drums, horns, low, tune = (Mix(song.loop + song.tail) for _ in range(4))
    chords = ["C4 E4 G4", "F3 A3 C4", "G3 B3 D4", "C4 E4 G4"]
    roots = ["C2", "F1", "G1", "C2"]
    k, sn, ch, cr = kick(rng), snare(rng), hat(rng), crash(rng)
    for bar in range(song.bars):
        index = bar % 4
        drums.add(k, song.at(bar, 0), 0.8)
        drums.add(k, song.at(bar, 2), 0.8)
        drums.add(k, song.at(bar, 2.5), 0.5)
        drums.add(sn, song.at(bar, 1), 0.5)
        drums.add(sn, song.at(bar, 3), 0.5)
        for beat in range(8):
            drums.add(ch, song.at(bar, beat / 2), 0.2, 0.3)
        if bar % 4 == 0:
            drums.add(cr, song.at(bar), 0.3, -0.4)
        for beat, length in ((0, 1.5), (1.5, 0.5), (2, 2)):
            voicing = sum(brass(f, song.beat * length * 0.95, 0.8) for f in chord(chords[index]))
            horns.add(voicing, song.at(bar, beat), 0.12)
        for eighth in range(8):
            low.add(bass(note(roots[index]), song.beat * 0.4, 800), song.at(bar, eighth / 2), 0.5)
    fanfare = [("G4", 0, 0.5), ("C5", 0.5, 0.5), ("E5", 1, 0.5), ("G5", 1.5, 1.5), ("E5", 3, 0.5), ("G5", 3.5, 0.5)]
    answer = [("A5", 0, 1), ("G5", 1, 1), ("F5", 2, 0.5), ("E5", 2.5, 0.5), ("D5", 3, 1)]
    close = [("E5", 0, 0.5), ("D5", 0.5, 0.5), ("C5", 1, 3)]
    for bar in range(8, 16):
        phrase = (fanfare, answer, fanfare, close)[(bar - 8) % 4] if bar % 2 == 0 or bar == 15 else []
        if bar % 2 == 1 and bar != 15:
            phrase = [("G5", 0, 2), ("E5", 2, 2)] if bar < 12 else [("F5", 0, 2), ("D5", 2, 2)]
        if bar == 15:
            phrase = close
        for name, beat, length in phrase:
            tune.add(brass(note(name), song.beat * length * 0.95, 1.0), song.at(bar, beat), 0.2)
    music = drums.buffer + low.buffer
    music += reverb(horns.buffer + tune.buffer, 1.8, 0.3, rng)[:, : drums.buffer.shape[1]]
    return music, song.loop


# --- Effects ---------------------------------------------------------------


def arpeggio(make: Callable[[float, float], Signal], names: Sequence[str], step: float, hold: float, seconds: float) -> Signal:
    out = Mix(seconds)
    for i, name in enumerate(names):
        out.add(make(note(name), hold), i * step, 1.0, -0.3 + 0.6 * i / max(1, len(names) - 1))
    return out.buffer


def start_gong(rng: np.random.Generator) -> Signal:
    return reverb(gong(note("A2"), 3.2, rng), 1.5, 0.25, rng)


def start_chime(rng: np.random.Generator) -> Signal:
    return reverb(arpeggio(lambda f, s: bell(f, s), ["E6", "B6"], 0.12, 1.2, 1.4), 1.2, 0.3, rng)


def start_go(rng: np.random.Generator) -> Signal:
    out = Mix(1.2)
    out.add(kick(rng), 0.0, 0.8)
    out.add(sum(brass(f, 0.25, 1.0) for f in chord("C4 G4 C5 E5")), 0.0, 0.35)
    out.add(crash(rng, 1.0), 0.0, 0.15)
    return reverb(out.buffer, 1.0, 0.25, rng)


def countdown(tick: Callable[[int], Signal], rng: np.random.Generator) -> Signal:
    """Five ticks a second apart, the last second's tick ringing up to the time up at 5 s."""
    out = Mix(5.0)
    for i in range(5):
        out.add(tick(i), float(i), 1.0)
    return reverb(out.buffer, 0.8, 0.15, rng)[:, : samples(5.0)] * ending(samples(5.0), 0.05)


def countdown_ticks(rng: np.random.Generator) -> Signal:
    return countdown(lambda i: woodblock(1800 if i % 2 == 0 else 1350, rng) * (0.7 + 0.075 * i), rng)


def countdown_beeps(rng: np.random.Generator) -> Signal:
    def beep(i: int) -> Signal:
        return square(1000 if i < 4 else 1500, 0.12, 9) * adsr(0.1, 0.003, 1, 1, 0.02) * 0.6

    return countdown(beep, rng)


def countdown_pulse(rng: np.random.Generator) -> Signal:
    def pulse(i: int) -> Signal:
        beat = Mix(0.6)
        beat.add(tom(55 + 6 * i, rng, 0.35), 0.0, 1.0)
        beat.add(tom(52 + 6 * i, rng, 0.35), 0.18, 0.7)
        return beat.buffer.mean(axis=0)

    out = countdown(pulse, rng)
    t = times(5.0)
    rise = sweep(noise(5.0, rng), lambda x: 300 * 2 ** (x * 1.1), 1.0) * np.clip((t - 2.5) / 2.5, 0, 1) ** 2 * ending(len(t), 0.05) * 0.25
    return out + stereo(rise)


def buzz_classic(rng: np.random.Generator) -> Signal:
    tone = (square(note("A5"), 0.32, 12) + 0.7 * square(note("E6"), 0.32, 8)) * adsr(0.28, 0.003, 0.2, 0.8, 0.04)
    return reverb(saturate(tone * 0.5, 1.5), 0.6, 0.15, rng)


def buzz_bell(rng: np.random.Generator) -> Signal:
    partials = ((1, 1, 0.9), (2.0, 0.3, 0.5), (2.76, 0.4, 0.4), (5.4, 0.2, 0.15))
    return reverb(bell(2093, 1.3, partials), 0.9, 0.2, rng)


def buzz_laser(rng: np.random.Generator) -> Signal:
    t = times(0.3)
    tone = saw(300 + 1700 * np.exp(-t / 0.06), 0.3, 20) * decay(0.3, 0.1, 0.002) * 0.6
    return echo(tone, 0.09, 0.3, 3)


def buzz_horn(rng: np.random.Generator) -> Signal:
    tone = lowpass(saw(note("A4"), 0.45, 30) + saw(note("C#5"), 0.45, 30), 2500) * adsr(0.39, 0.01, 0.2, 0.9, 0.06) * 0.4
    return reverb(tone, 0.6, 0.15, rng)


def correct_chime(rng: np.random.Generator) -> Signal:
    return reverb(arpeggio(lambda f, s: bell(f, s), ["C6", "E6", "G6", "C7"], 0.08, 1.2, 1.6), 1.2, 0.3, rng)


def correct_dings(rng: np.random.Generator) -> Signal:
    partials = ((1, 1, 0.8), (2.0, 0.4, 0.4), (3.0, 0.2, 0.2), (4.2, 0.1, 0.1))
    out = Mix(1.4)
    out.add(bell(note("E6"), 1.0, partials), 0.0, 0.9, -0.2)
    out.add(bell(note("E6"), 1.0, partials), 0.16, 0.9, 0.2)
    return reverb(out.buffer, 1.0, 0.25, rng)


def correct_fanfare(rng: np.random.Generator) -> Signal:
    out = Mix(1.8)
    for name, at, length in (("G4", 0.0, 0.12), ("C5", 0.15, 0.12), ("E5", 0.3, 0.12), ("G5", 0.45, 0.9)):
        out.add(brass(note(name), length, 1.0), at, 0.6)
    out.add(sum(brass(f, 0.9, 0.7) for f in chord("C4 E4")), 0.45, 0.3)
    return reverb(out.buffer, 1.4, 0.3, rng)


def wrong_buzzer(rng: np.random.Generator) -> Signal:
    tone = saw(note("A#2"), 0.75, 40) + saw(cents(note("A#2"), 40), 0.75, 40) + 0.5 * square(note("F2"), 0.75, 30)
    return reverb(saturate(lowpass(tone, 2200) * adsr(0.69, 0.005, 0.3, 0.9, 0.06) * 0.35, 2.0), 0.6, 0.1, rng)


def wrong_trombone(rng: np.random.Generator) -> Signal:
    out = Mix(2.6)
    for i, (name, length) in enumerate((("A#3", 0.35), ("A3", 0.35), ("G#3", 0.35), ("G3", 1.2))):
        f = note(name)
        t = times(length + 0.12)
        slide = f * (1 + 0.012 * np.sin(2 * np.pi * 5 * t) * (i == 3) * np.minimum(1, t / 0.4))
        tone = lowpass(additive(slide, length + 0.12, lambda k: np.exp(-(k - 1) / 4) / k**0.4, 24), 1800)
        out.add(tone * adsr(length, 0.03, 0.2, 0.8, 0.12) * 0.4, i * 0.42)
    return reverb(out.buffer, 1.2, 0.2, rng)


def wrong_bonk(rng: np.random.Generator) -> Signal:
    out = Mix(0.9)
    out.add(tom(180, rng, 0.35), 0.0, 0.8)
    out.add(tom(120, rng, 0.45), 0.18, 0.9)
    return reverb(out.buffer, 0.7, 0.15, rng)


def timeup_horn(rng: np.random.Generator) -> Signal:
    tone = sum(saw(f, 1.3, 40) for f in chord("D4 F#4 A4")) + saw(note("D3"), 1.3, 40)
    return reverb(lowpass(tone, 3000) * adsr(1.15, 0.03, 0.4, 0.85, 0.15) * 0.22, 0.9, 0.2, rng)


def timeup_gong(rng: np.random.Generator) -> Signal:
    return reverb(gong(note("D2"), 4.5, rng), 2.0, 0.3, rng)


def timeup_alarm(rng: np.random.Generator) -> Signal:
    t = times(1.3)
    ring = bell(1250, 1.3, ((1, 1, 2.0), (2.76, 0.4, 1.0), (5.4, 0.2, 0.5)))
    strikes = 0.55 + 0.45 * np.sign(np.sin(2 * np.pi * 18 * t))
    return reverb(ring * strikes * np.clip((1.3 - t) / 0.2, 0, 1), 0.7, 0.2, rng)


def reveal_whoosh(rng: np.random.Generator) -> Signal:
    t = times(1.2)
    air = sweep(noise(1.2, rng), lambda x: 400 * 2 ** (np.minimum(x, 0.5) * 9), 1.5)
    shape = np.sin(np.pi * np.clip(t / 0.75, 0, 1)) ** 2
    out = Mix(1.6)
    out.add(air * shape * 0.8, 0.0)
    out.add(arpeggio(lambda f, s: bell(f, s) * 0.3, ["G6", "D7"], 0.05, 0.8, 1.0), 0.55)
    return reverb(out.buffer, 1.0, 0.25, rng)


def reveal_sparkle(rng: np.random.Generator) -> Signal:
    names = ["C6", "E6", "G6", "B6", "D7", "G7"]
    return reverb(arpeggio(lambda f, s: bell(f, s) * 0.5, names, 0.045, 0.9, 1.6), 1.6, 0.4, rng)


def reveal_sting(rng: np.random.Generator) -> Signal:
    out = Mix(2.0)
    out.add(sum(brass(f, 0.6, 1.0) for f in chord("A#3 D4 F4 A#4")), 0.0, 0.3)
    out.add(timpani(note("A#1"), rng, 1.5), 0.0, 0.6)
    out.add(crash(rng, 1.8), 0.0, 0.2)
    return reverb(out.buffer, 1.6, 0.3, rng)


def leaderboard_rise(rng: np.random.Generator) -> Signal:
    names = ["C4", "E4", "G4", "C5", "E5", "G5"]
    out = Mix(2.2)
    out.add(arpeggio(lambda f, s: pluck(f, s, 1.2), names, 0.07, 0.25, 1.0), 0.0, 0.7)
    out.add(pad(chord("C4 E4 G4 C5"), 0.9, 2500), 0.42, 1.0)
    return reverb(out.buffer, 1.4, 0.3, rng)


def leaderboard_drums(rng: np.random.Generator) -> Signal:
    out = Mix(2.2)
    for i, f in enumerate((220, 196, 165, 147, 131, 110)):
        out.add(tom(f, rng, 0.4), i * 0.075, 0.6, 0.5 - 0.2 * i)
    out.add(kick(rng), 0.45, 0.9)
    out.add(crash(rng, 1.7), 0.45, 0.3)
    return reverb(out.buffer, 1.0, 0.2, rng)


def leaderboard_chord(rng: np.random.Generator) -> Signal:
    stab = sum(brass(f, 0.5, 1.0) for f in chord("F3 C4 F4 A4 C5"))
    return reverb(echo(stab * 0.3, 0.2, 0.3, 2), 1.6, 0.35, rng)


def place_cymbal(rng: np.random.Generator) -> Signal:
    out = Mix(2.6)
    out.add(crash(rng, 2.4), 0.0, 0.5)
    out.add(timpani(note("C2"), rng, 1.6), 0.0, 0.7)
    return reverb(out.buffer, 1.4, 0.2, rng)


def place_stab(rng: np.random.Generator) -> Signal:
    out = Mix(1.4)
    out.add(sum(brass(f, 0.35, 1.0) for f in chord("D4 F#4 A4 D5")), 0.0, 0.32)
    out.add(kick(rng), 0.0, 0.7)
    return reverb(out.buffer, 1.2, 0.3, rng)


def place_boom(rng: np.random.Generator) -> Signal:
    t = times(1.8)
    drop = np.sin(phase(30 + 80 * np.exp(-t / 0.15), len(t))) * np.exp(-t / 0.6)
    hit = lowpass(noise(1.8, rng), 800) * np.exp(-t / 0.05) * 0.5
    return reverb(saturate(drop + hit, 1.4) * ending(len(t)), 1.8, 0.3, rng)


def winner_fanfare(rng: np.random.Generator) -> Signal:
    out = Mix(4.4)
    tune = [("C5", 0.0, 0.14), ("C5", 0.17, 0.14), ("C5", 0.34, 0.14), ("C5", 0.51, 0.45), ("G#4", 1.0, 0.45), ("A#4", 1.5, 0.45), ("C5", 2.0, 0.25), ("A#4", 2.3, 0.15), ("C5", 2.5, 1.5)]
    for name, at, length in tune:
        out.add(brass(note(name), length, 1.0), at, 0.45)
        out.add(brass(note(name) / 2, length, 0.7), at, 0.25)
    out.add(sum(brass(f, 1.5, 0.8) for f in chord("C4 E4 G4")), 2.5, 0.18)
    out.add(timpani(note("C2"), rng, 2.0), 2.5, 0.6)
    out.add(crash(rng, 2.0), 2.5, 0.25)
    return reverb(out.buffer, 1.8, 0.3, rng)


def winner_applause(rng: np.random.Generator) -> Signal:
    seconds = 4.5
    out = Mix(seconds)
    t = times(seconds)
    density = np.interp(np.arange(0, seconds, 0.01), [0, 0.4, 2.8, seconds], [0, 1, 1, 0])
    for i, d in enumerate(density):
        for _ in range(rng.poisson(0.5 * d)):
            burst = bandpass(noise(0.06, rng), rng.uniform(700, 1500), rng.uniform(2200, 4500)) * decay(0.06, rng.uniform(0.006, 0.014), 0.001)
            out.add(burst, i * 0.01 + rng.uniform(0, 0.01), rng.uniform(0.25, 0.6), rng.uniform(-0.9, 0.9))
    crowd = bandpass(noise(seconds, rng), 500, 3000) * np.interp(t, [0, 0.5, 2.8, seconds], [0, 0.12, 0.1, 0])
    out.add(stereo(crowd), 0.0)
    return reverb(out.buffer, 1.2, 0.25, rng)


def winner_party(rng: np.random.Generator) -> Signal:
    out = Mix(3.6)
    names = ["C5", "E5", "G5", "C6", "G5", "C6", "E6", "G6"]
    out.add(arpeggio(lambda f, s: pluck(f, s, 1.2) + 0.3 * bell(f, s + 0.12), names, 0.09, 0.3, 1.2), 0.0, 0.6)
    for i in range(4):
        out.add(kick(rng), i * 0.27, 0.6)
        out.add(snare(rng), i * 0.27 + 0.135, 0.3)
    out.add(sum(brass(f, 1.6, 0.9) for f in chord("C4 E4 G4 C5")), 1.08, 0.18)
    out.add(crash(rng, 2.2), 1.08, 0.35)
    return reverb(out.buffer, 1.6, 0.3, rng)


# --- The library -----------------------------------------------------------

EFFECTS: dict[str, Callable[[np.random.Generator], Signal]] = {
    "start.gong": start_gong,
    "start.chime": start_chime,
    "start.go": start_go,
    "countdown.ticks": countdown_ticks,
    "countdown.beeps": countdown_beeps,
    "countdown.pulse": countdown_pulse,
    "buzz.classic": buzz_classic,
    "buzz.bell": buzz_bell,
    "buzz.laser": buzz_laser,
    "buzz.horn": buzz_horn,
    "correct.chime": correct_chime,
    "correct.dings": correct_dings,
    "correct.fanfare": correct_fanfare,
    "wrong.buzzer": wrong_buzzer,
    "wrong.trombone": wrong_trombone,
    "wrong.bonk": wrong_bonk,
    "timeup.horn": timeup_horn,
    "timeup.gong": timeup_gong,
    "timeup.alarm": timeup_alarm,
    "reveal.whoosh": reveal_whoosh,
    "reveal.sparkle": reveal_sparkle,
    "reveal.sting": reveal_sting,
    "leaderboard.rise": leaderboard_rise,
    "leaderboard.drums": leaderboard_drums,
    "leaderboard.chord": leaderboard_chord,
    "place.cymbal": place_cymbal,
    "place.stab": place_stab,
    "place.boom": place_boom,
    "winner.fanfare": winner_fanfare,
    "winner.applause": winner_applause,
    "winner.party": winner_party,
}

MUSIC: dict[str, Callable[[np.random.Generator], tuple[Signal, float]]] = {
    "music.lobby": lobby,
    "music.lounge": lounge,
    "music.thinking": thinking,
    "music.race": race,
    "music.drumroll": drumroll,
    "music.victory": victory,
}


# --- Loudness and files ----------------------------------------------------


def k_weighting(f: Signal) -> Signal:
    """The magnitude of BS.1770-4's K filter (its two 48 kHz biquads) at `f`."""
    z = np.exp(-2j * np.pi * f / SR)

    def biquad(b: Sequence[float], a: Sequence[float]) -> Signal:
        return np.abs((b[0] + b[1] * z + b[2] * z**2) / (a[0] + a[1] * z + a[2] * z**2))

    shelf = biquad((1.53512485958697, -2.69169618940638, 1.19839281085285), (1.0, -1.69065929318241, 0.73248077421585))
    rlb = biquad((1.0, -2.0, 1.0), (1.0, -1.99004745483398, 0.99007225036621))
    return shelf * rlb


def loudness(signal: Signal) -> float:
    """Integrated loudness in LUFS (BS.1770-4: 400 ms blocks, gates at -70 LUFS and -10 LU)."""
    weighted = np.array([spectral(channel, k_weighting) for channel in signal])
    block, hop = samples(0.4), samples(0.1)
    if weighted.shape[1] < block:
        weighted = np.pad(weighted, ((0, 0), (0, block - weighted.shape[1])))
    powers = np.array([np.sum(np.mean(weighted[:, s : s + block] ** 2, axis=1)) for s in range(0, weighted.shape[1] - block + 1, hop)])
    lufs = -0.691 + 10 * np.log10(np.maximum(powers, 1e-12))
    gated = powers[lufs > -70]
    if len(gated) == 0:
        return -70.0
    relative = -0.691 + 10 * np.log10(np.mean(gated)) - 10
    kept = powers[(lufs > -70) & (lufs > relative)]
    return float(-0.691 + 10 * np.log10(np.mean(kept)))


def level(signal: Signal) -> tuple[Signal, float, float]:
    """`signal` at `TARGET_LUFS`, or lower where its peak would pass `PEAK_DB`; with its loudness and peak."""
    out = as_stereo(signal)
    # Twice: the -70 LUFS gate lets in more of a quiet tail once it is louder.
    for _ in range(2):
        gain_db = min(TARGET_LUFS - loudness(out), PEAK_DB - 20 * np.log10(np.max(np.abs(out))))
        out = out * 10 ** (gain_db / 20)
    fade = min(samples(0.01), out.shape[1])
    out[:, -fade:] *= np.linspace(1, 0, fade)
    return out, loudness(out), 20 * np.log10(np.max(np.abs(out)))


def encode(pcm: Signal, path: Path) -> None:
    frames = (np.clip(pcm, -1, 1).T * 32767).astype("<i2").tobytes()
    with tempfile.TemporaryDirectory() as folder:
        wav = Path(folder) / "sound.wav"
        with wave.open(str(wav), "wb") as out:
            out.setnchannels(pcm.shape[0])
            out.setsampwidth(2)
            out.setframerate(SR)
            out.writeframes(frames)
        command = ["ffmpeg", "-v", "error", "-y", "-i", str(wav), "-codec:a", "libmp3lame", "-q:a", MP3_QUALITY]
        subprocess.run([*command, "-map_metadata", "-1", "-id3v2_version", "0", "-write_xing", "1", str(path)], check=True)


def decoded_peak_db(path: Path) -> float:
    command = ["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-"]
    decoded = np.frombuffer(subprocess.run(command, capture_output=True, check=True).stdout, dtype="<f4")
    return float(20 * np.log10(np.max(np.abs(decoded))))


def write_mp3(signal: Signal, path: Path, mono: bool) -> float:
    """Writes `signal` as MP3, lowered until its decoded peak stays under `PEAK_DB`
    (the encoder overshoots on sharp attacks); returns the gain it took, in dB."""
    pcm = signal.mean(axis=0, keepdims=True) if mono else signal
    lowered = 0.0
    for _ in range(4):
        encode(pcm * 10 ** (lowered / 20), path)
        over = decoded_peak_db(path) - PEAK_DB
        if over <= 0:
            break
        lowered -= over + 0.1
    return lowered


def seed(sound_id: str) -> np.random.Generator:
    return np.random.default_rng(zlib.crc32(sound_id.encode()))


def main() -> None:
    manifest = []
    for sound_id, make in EFFECTS.items():
        signal, lufs, peak = level(make(seed(sound_id)))
        # Effects are near enough mono that one channel halves the file.
        lowered = write_mp3(signal, HERE / f"{sound_id}.mp3", mono=True)
        manifest.append({"id": sound_id, "kind": "effect", "ms": round(signal.shape[1] * 1000 / SR)})
        print(f"{sound_id:20} {signal.shape[1] / SR:5.2f} s {lufs + lowered:6.1f} LUFS {peak:5.1f} dBFS", file=sys.stderr)
    for sound_id, make in MUSIC.items():
        music, loop = make(seed(sound_id))
        signal, lufs, peak = level(music)
        lufs += write_mp3(signal, HERE / f"{sound_id}.mp3", mono=False)
        manifest.append({"id": sound_id, "kind": "music", "ms": round(signal.shape[1] * 1000 / SR), "loopMs": round(loop * 1000)})
        print(f"{sound_id:20} {signal.shape[1] / SR:5.2f} s {lufs:6.1f} LUFS {peak:5.1f} dBFS loop {loop:.2f} s", file=sys.stderr)
    (HERE / "sounds.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
