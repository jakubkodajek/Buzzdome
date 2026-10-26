// The quiz's sounds (spec SN-1 to SN-4): what each moment of the show
// plays, tried out here as loud as the show plays it.

import { useApp } from "../app-context";
import { EFFECT_SLOTS, MAX_SOUND_VOLUME, MUSIC_SLOTS } from "../bindings/Constants";
import type { Translate } from "../i18n";
import { PlayIcon, StopIcon } from "../icons";
import { mediaFileName } from "../media";
import { choiceKey, choiceOf, defaultChoice, isMusic, LIBRARY, sameChoice, SLOT_SOUNDS, SOUND_NAME, slotLabel, withChoice } from "../audio/library";
import { useSoundPreview, type SoundPreview } from "../audio/show-sounds";
import type { QuizSounds, SoundChoice, SoundSlot } from "../types";
import { NumberBox, Select, type OptionGroup } from "./controls";

/** The value of the entry that picks a file of the quiz's own. */
const PICK_FILE = "pick";

interface SoundsSettingsProps {
  sounds: QuizSounds;
  onSounds: (sounds: QuizSounds) => void;
  /** Lets the quizmaster pick a sound file, added to the quiz's media; null when none is picked. */
  onPickFile: () => Promise<string | null>;
}

export function SoundsSettings({ sounds, onSounds, onPickFile }: SoundsSettingsProps) {
  const { t } = useApp();
  const preview = useSoundPreview(sounds);
  const choose = (slot: SoundSlot, choice: SoundChoice) => {
    onSounds(withChoice(sounds, slot, choice));
    // Heard as it is picked.
    if (choice === "none") preview.stop();
    else preview.play(slot, choice);
  };
  const rows = (slots: readonly SoundSlot[]) => (
    <div className="sound-rows">
      {slots.map((slot) => (
        <SoundRow key={slot} slot={slot} choice={choiceOf(sounds, slot)} preview={preview} onChoose={(choice) => choose(slot, choice)} onPickFile={onPickFile} />
      ))}
    </div>
  );
  const volume = (label: string, value: number, change: (value: number) => void) => (
    <div className="field">
      <span>{label}</span>
      <NumberBox label={label} value={value} min={0} max={MAX_SOUND_VOLUME} unit="%" slider onChange={change} />
    </div>
  );
  return (
    <>
      <h2>{t("soundsAndMusic")}</h2>
      <p className="backstage-hint">{t("soundsHint")}</p>
      <h3>{t("soundEffects")}</h3>
      {rows(EFFECT_SLOTS)}
      {volume(t("effectsVolume"), sounds.effectsVolume, (effectsVolume) => onSounds({ ...sounds, effectsVolume }))}
      <h3>{t("soundMusic")}</h3>
      {rows(MUSIC_SLOTS)}
      {volume(t("musicVolume"), sounds.musicVolume, (musicVolume) => onSounds({ ...sounds, musicVolume }))}
    </>
  );
}

function SoundRow(props: { slot: SoundSlot; choice: SoundChoice; preview: SoundPreview; onChoose: (choice: SoundChoice) => void; onPickFile: () => Promise<string | null> }) {
  const { slot, choice, preview, onChoose, onPickFile } = props;
  const { t } = useApp();
  const label = slotLabel(t, slot);
  const playing = preview.playing === slot;
  const choices = new Map<string, SoundChoice>();
  const entry = (choice: SoundChoice, text: string): [string, string] => {
    choices.set(choiceKey(choice), choice);
    return [choiceKey(choice), text];
  };
  const builtIn = (id: string) => entry({ builtIn: id }, soundName(t, slot, id));
  const ownFile = typeof choice === "object" && "file" in choice ? [entry(choice, t("soundOwnFile", { name: mediaFileName(choice.file) }))] : [];
  const options: ([string, string] | OptionGroup<string>)[] = [
    entry("none", t("soundNone")),
    { group: t("soundMadeFor"), options: SLOT_SOUNDS[slot].madeFor.map(builtIn) },
    ...otherSounds(t, slot).map(({ group, ids }) => ({ group, options: ids.map(builtIn) })),
    ...ownFile,
    [PICK_FILE, t("soundChooseFile")],
  ];
  const pick = (key: string) => {
    const picked = choices.get(key);
    if (picked) onChoose(picked);
    else void onPickFile().then((file) => file && onChoose({ file }));
  };
  return (
    <>
      <span>{label}</span>
      <Select label={label} value={choiceKey(choice)} options={options} onChange={pick} />
      <button
        type="button"
        className="mini"
        aria-label={`${label}: ${t(playing ? "stopSound" : "playSound")}`}
        title={t(playing ? "stopSound" : "playSound")}
        disabled={choice === "none"}
        onClick={() => (playing ? preview.stop() : preview.play(slot))}
      >
        {playing ? <StopIcon /> : <PlayIcon />}
      </button>
    </>
  );
}

/** What one of the app's sounds is called in `slot`'s list: its name, said to be the default if it is. */
function soundName(t: Translate, slot: SoundSlot, id: string): string {
  const name = t(SOUND_NAME[id]);
  return sameChoice(defaultChoice(slot), { builtIn: id }) ? t("soundDefault", { name }) : name;
}

/**
 * The app's other sounds `slot` can play, under headings: for an effect,
 * those made for each other effect; for music, all the other music.
 */
function otherSounds(t: Translate, slot: SoundSlot): { group: string; ids: readonly string[] }[] {
  const { madeFor } = SLOT_SOUNDS[slot];
  if (isMusic(slot)) return [{ group: t("soundOtherMusic"), ids: LIBRARY.filter((sound) => sound.kind === "music" && !madeFor.includes(sound.id)).map((sound) => sound.id) }];
  return EFFECT_SLOTS.filter((other) => other !== slot).map((other) => ({ group: slotLabel(t, other), ids: SLOT_SOUNDS[other].madeFor }));
}
