// The show's sounds in the Director (spec SN-1 to SN-4): each moment's
// effect when the game sounds its cue, the countdown over a running
// question's last seconds, and the music the show's moment takes; and
// trying a moment's sound out in the editor.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { DEFAULT_QUIZ_SOUNDS, LAST_SECONDS_MS, SOUND_SLOTS } from "../bindings/Constants";
import { useBackendEvent, useClock, usePlayerVolume } from "../hooks";
import { useMediaUrl } from "../media";
import { heardAtNow, msSinceHeard } from "../media-player";
import { soundsOf } from "../quiz";
import type { GameView, NowPlaying, Quiz, QuizSounds, SoundChoice, SoundSlot } from "../types";
import { linearGain, playerLevel } from "./clip-player";
import { choiceOf, EFFECT_SLOTS, MUSIC_SLOTS, SLOT_SOUNDS, soundSource, type SoundSource } from "./library";
import { ShowAudio, type Playing } from "./show-audio";

type SlotSources = Record<SoundSlot, SoundSource | null>;

/** What each moment plays by the quiz's `sounds`, its own files found in the media folder of the window. */
function useSlotSources(sounds: QuizSounds): SlotSources {
  const fileUrl = useMediaUrl();
  return useMemo(
    () => Object.fromEntries(SOUND_SLOTS.map((slot) => [slot, soundSource(slot, choiceOf(sounds, slot), fileUrl)])) as SlotSources,
    [sounds, fileUrl],
  );
}

/**
 * Audio to play the quiz's `sounds` with while mounted, at their volumes
 * times the player's volume. It is made by the first effect, so the
 * effects after it find it.
 */
function useShowAudio(sounds: QuizSounds): RefObject<ShowAudio | null> {
  const audio = useRef<ShowAudio | null>(null);
  useEffect(() => {
    const made = new ShowAudio();
    audio.current = made;
    return () => {
      made.dispose();
      audio.current = null;
    };
  }, []);
  const level = playerLevel(usePlayerVolume());
  const { effectsVolume, musicVolume } = sounds;
  useEffect(() => audio.current?.setVolumes(linearGain(effectsVolume, 0) * level, linearGain(musicVolume, 0) * level), [effectsVolume, musicVolume, level]);
  return audio;
}

/**
 * Plays the show's sounds as `quiz` says: each cue's effect, the
 * countdown, and the music of the show's moment, taken away while `ducked`
 * (other sound plays, see `otherSoundPlays`).
 */
export function useShowSounds(view: GameView | null, quiz: Quiz | null, ducked: boolean) {
  const sounds = quiz ? soundsOf(quiz) : DEFAULT_QUIZ_SOUNDS;
  const sources = useSlotSources(sounds);
  const audio = useShowAudio(sounds);

  useEffect(() => {
    const found = (slots: readonly SoundSlot[]) => slots.flatMap((slot) => sources[slot] ?? []);
    audio.current?.prepare(found(EFFECT_SLOTS), found(MUSIC_SLOTS));
  }, [audio, sources]);

  useBackendEvent("sound-cue", (cue) => {
    const source = sources[cue];
    if (source) audio.current?.effect(source);
  });

  const music = view?.music ?? null;
  useEffect(() => audio.current?.music(music && sources[music]), [audio, music, sources]);
  useEffect(() => audio.current?.duck(ducked), [audio, ducked]);
  useCountdown(audio, view, sources.countdown);
}

/**
 * Whether sound other than the show's plays: the slide's sound, a sound or
 * a video the media player plays (`now`), or a slide video with sound,
 * which plays on the projector while the slide is up. The music makes way
 * for it.
 */
export function otherSoundPlays(view: GameView | null, slideSoundPlays: boolean, now: NowPlaying | null): boolean {
  const video = view?.slide?.video ?? null;
  const mediaPlays = now !== null && now.kind !== "image" && now.sinceMs !== null;
  return slideSoundPlays || mediaPlays || (video !== null && !video.muted && video.volume > 0);
}

/**
 * Plays `source` over the last `LAST_SECONDS_MS` of a running question's
 * clock, in time with it, so that it ends as the time is up. A pause or a
 * buzz stops it, and the clock going on starts it where the clock is.
 */
function useCountdown(audio: RefObject<ShowAudio | null>, view: GameView | null, source: SoundSource | null) {
  const clock = useClock(view);
  // When the latest reading of the clock arrived, to count on from it.
  const reading = useRef({ remainingMs: null as number | null, heardAt: 0 });
  useLayoutEffect(() => {
    reading.current = { remainingMs: clock?.remainingMs ?? null, heardAt: heardAtNow() };
  }, [clock]);
  const running = view?.phase.phase === "question" && view.phase.step === "running" && (clock?.running ?? false);
  const timed = (clock?.limitMs ?? null) !== null;

  useEffect(() => {
    if (!source || !running || !timed) return;
    const leftMs = () => {
      const { remainingMs, heardAt } = reading.current;
      return remainingMs === null ? 0 : remainingMs - msSinceHeard(heardAt);
    };
    let playing: Playing | null = null;
    const start = () => {
      const left = leftMs();
      if (left > 0) playing = audio.current?.inTime(source, Math.max(0, LAST_SECONDS_MS - left)) ?? null;
    };
    const wait = setTimeout(start, Math.max(0, leftMs() - LAST_SECONDS_MS));
    return () => {
      clearTimeout(wait);
      playing?.stop();
    };
  }, [audio, source, running, timed]);
}

export interface SoundPreview {
  /** The moment whose sound plays, if any. */
  playing: SoundSlot | null;
  /** Plays what `slot` plays, or `choice` for it, instead of what plays. */
  play: (slot: SoundSlot, choice?: SoundChoice) => void;
  stop: () => void;
}

/** Tries out the sound each moment takes by the quiz's `sounds`, as loud as the show plays it. */
export function useSoundPreview(sounds: QuizSounds): SoundPreview {
  const fileUrl = useMediaUrl();
  const audio = useShowAudio(sounds);
  const [playing, setPlaying] = useState<SoundSlot | null>(null);
  const effect = useRef<Playing | null>(null);

  const stop = useCallback(() => {
    effect.current?.stop();
    effect.current = null;
    audio.current?.music(null);
    setPlaying(null);
  }, [audio]);

  const play = useCallback(
    (slot: SoundSlot, choice = choiceOf(sounds, slot)) => {
      stop();
      const player = audio.current;
      const source = soundSource(slot, choice, fileUrl);
      if (!player || !source) return;
      setPlaying(slot);
      if (SLOT_SOUNDS[slot].music) {
        player.music(source);
        return;
      }
      const played = player.effect(source, () => {
        if (effect.current !== played) return;
        effect.current = null;
        setPlaying(null);
      });
      effect.current = played;
    },
    [audio, sounds, fileUrl, stop],
  );

  return { playing, play, stop };
}
