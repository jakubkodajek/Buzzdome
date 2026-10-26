import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { LAST_SECONDS_MS } from "../bindings/Constants";
import {
  BUZZ_NOTE,
  FIRST_CUE_NOTE,
  FIRST_STAGE_NOTE,
  MAX_ARTNET_UNIVERSE,
  MAX_EFFECT_MS,
  MAX_FADE_MS,
  MAX_FIXTURE_CHANNELS,
  MAX_FIXTURES,
  MAX_FLASH_MS,
  MAX_LIGHTS_NAME_CHARS,
  MAX_LOOK_COLORS,
  MAX_LOOKS,
  MAX_SACN_UNIVERSE,
  MIN_EFFECT_MS,
  MIN_FLASH_MS,
  MIN_SACN_UNIVERSE,
  SHOW_ENDED_NOTE,
  SHOW_STARTED_NOTE,
  UNIVERSE_SIZE,
} from "../bindings/LightsConstants";
import { ColorBox } from "../editor/ColorPicker";
import { NumberBox, Select, Switch } from "../editor/controls";
import { calendarDay } from "../format";
import { useLightsStatus } from "../hooks";
import type { StringKey, Translate } from "../i18n";
import { errorText, outputErrorText, settingsLostText, setupErrorText } from "../messages";
import { CUE_LABEL } from "../phase";
import type { Channel, Cue, Fixture, LibraryFixture, LibraryInfo, LibraryMatches, LibraryMode, LightsDevices, LightsInfo, LightsSetup, Look, LookEffect, OutputChoice, Stage } from "../types";
import {
  addFixture,
  addLook,
  changeFixture,
  changeLook,
  channelOf,
  CHANNEL_KINDS,
  FIXTURE_PRESETS,
  fixtureColor,
  followsLooks,
  formatChannelValues,
  outputOfKind,
  overlapping,
  parseChannelValues,
  removeFixture,
  removeLook,
  sameOutput,
} from "./lights";

/** How long the tab waits after the last change before it keeps the setup. */
const STORE_AFTER_MS = 400;
/** How often the preview reads the channels. */
const PREVIEW_EVERY_MS = 250;

const OUTPUT_KINDS: [OutputChoice["kind"], StringKey][] = [
  ["none", "lightsOutputNone"],
  ["enttecPro", "lightsOutputEnttecPro"],
  ["openDmx", "lightsOutputOpenDmx"],
  ["artNet", "lightsOutputArtNet"],
  ["sacn", "lightsOutputSacn"],
  ["soundSwitch", "lightsOutputSoundSwitch"],
];

const CHANNEL_KEYS: Record<Channel["kind"], StringKey> = {
  dimmer: "lightsChannelDimmer",
  dimmerRange: "lightsChannelDimmerRange",
  red: "lightsChannelRed",
  green: "lightsChannelGreen",
  blue: "lightsChannelBlue",
  white: "lightsChannelWhite",
  strobe: "lightsChannelStrobe",
  shutter: "lightsChannelShutter",
  fixed: "lightsChannelFixed",
  unused: "lightsChannelUnused",
};

const EFFECT_KEYS: Record<LookEffect, StringKey> = {
  static: "lightsEffectStatic",
  chase: "lightsEffectChase",
  pulse: "lightsEffectPulse",
  strobe: "lightsEffectStrobe",
};

const STAGE_KEYS: Record<Stage, StringKey> = {
  noShow: "lightsStageNoShow",
  lobby: "lightsStageLobby",
  billboard: "lightsStageBillboard",
  explanation: "lightsStageExplanation",
  endOfRound: "lightsStageEndOfRound",
  questionReady: "lightsStageQuestionReady",
  answering: "lightsStageAnswering",
  lastSeconds: "lightsStageLastSeconds",
  buzzed: "lightsStageBuzzed",
  closed: "lightsStageClosed",
  revealed: "lightsStageRevealed",
  leaderboard: "lightsStageLeaderboard",
  finalBuildUp: "lightsStageFinalBuildUp",
  winner: "lightsStageWinner",
};

const stageText = (t: Translate, stage: Stage) => t(STAGE_KEYS[stage], { s: LAST_SECONDS_MS / 1000 });

/**
 * The venue's stage lights (spec section 12): the DMX output, the fixtures,
 * the looks, which look each moment of the show takes, the messages for
 * other programs, and live control with a preview. Every change is kept a
 * moment after the last one, and the lights take it at once.
 */
export function LightsTab() {
  const { t } = useApp();
  const run = useRun();
  const [info, setInfo] = useState<LightsInfo | null>(null);
  const [setup, setSetup] = useState<LightsSetup | null>(null);
  const pending = useRef<{ setup: LightsSetup; timer: number } | null>(null);

  useEffect(() => {
    run(
      api.lightsInfo().then((loaded) => {
        setInfo(loaded);
        setSetup(loaded.setup);
      }),
    );
  }, [run]);

  // A setup kept is the one in use, so a setup refused at start no longer is the news.
  const store = useCallback((next: LightsSetup) => run(api.lightsStore(next).then(() => setInfo((i) => i && { ...i, refused: null }))), [run]);
  // A change still waiting is kept when the tab closes.
  useEffect(
    () => () => {
      if (pending.current) {
        window.clearTimeout(pending.current.timer);
        store(pending.current.setup);
      }
    },
    [store],
  );

  const change = useCallback(
    (edit: (setup: LightsSetup) => LightsSetup) => {
      setSetup((current) => {
        if (!current) return current;
        const next = edit(current);
        if (pending.current) window.clearTimeout(pending.current.timer);
        const timer = window.setTimeout(() => {
          pending.current = null;
          store(next);
        }, STORE_AFTER_MS);
        pending.current = { setup: next, timer };
        return next;
      });
    },
    [store],
  );

  if (!info || !setup) return <div className="lights" />;
  return (
    <div className="lights">
      <LiveControl setup={setup} change={change} />
      {info.settingsError && (
        <p className="receiver-state error">
          {t("lightsSettingsLost")} {settingsLostText(t, info.settingsError)}
        </p>
      )}
      {info.refused && <p className="receiver-state error">{t("lightsRefusedKept", { problem: setupErrorText(t, info.refused) })}</p>}
      <OutputSection output={setup.output} onChange={(output) => change((s) => ({ ...s, output }))} />
      <FixturesSection setup={setup} change={change} />
      <LooksSection setup={setup} change={change} />
      <WhenSection setup={setup} change={change} />
      <MessagesSection setup={setup} change={change} />
      <Preview fixtures={setup.fixtures} />
    </div>
  );
}

type Change = (edit: (setup: LightsSetup) => LightsSetup) => void;

/** The output's status, blackout, a look to hold and the master. */
function LiveControl({ setup, change }: { setup: LightsSetup; change: Change }) {
  const { t } = useApp();
  const run = useRun();
  const status = useLightsStatus();
  const lookName = (id: number | null) => setup.looks.find((look) => look.id === id)?.name ?? t("lightsDark");
  const output = status?.output;
  return (
    <fieldset>
      <legend>{t("lightsLive")}</legend>
      <p className={`receiver-state ${output?.kind === "connected" ? "connected" : output?.kind === "lost" ? "error" : ""}`}>
        {output?.kind === "off" && t("lightsOff")}
        {output?.kind === "connected" && t("lightsConnected", { name: output.name })}
        {output?.kind === "lost" && t("lightsLost", { problem: outputErrorText(t, output.error) })}
      </p>
      {status?.messages && <p className="receiver-state error">{t("lightsMessagesFailed", { problem: outputErrorText(t, status.messages) })}</p>}
      <div className="row">
        <button className={status?.blackout ? "blackout on" : "blackout"} aria-pressed={status?.blackout ?? false} onClick={() => run(api.lightsBlackout(!status?.blackout))}>
          {status?.blackout ? t("lightsBlackoutOn") : t("lightsBlackout")}
        </button>
        <label className="inline">
          <span>{t("lightsHold")}</span>
          <Select
            label={t("lightsHold")}
            value={status?.hold ?? 0}
            options={[[0, t("lightsFollowShow")], ...setup.looks.map((look): [number, string] => [look.id, look.name])]}
            onChange={(look) => run(api.lightsHold(look === 0 ? null : look))}
          />
        </label>
        <span className="muted">{t("lightsShowing", { look: lookName(status?.look ?? null) })}</span>
      </div>
      <label className="inline">
        <span>{t("lightsMaster")}</span>
        <NumberBox label={t("lightsMaster")} value={setup.master} min={0} max={100} unit="%" slider onChange={(master) => change((s) => ({ ...s, master }))} />
      </label>
    </fieldset>
  );
}

/** Where the universe goes, with the interfaces found on this computer. */
function OutputSection({ output, onChange }: { output: OutputChoice; onChange: (output: OutputChoice) => void }) {
  const { t } = useApp();
  const run = useRun();
  const [devices, setDevices] = useState<LightsDevices | null>(null);
  const find = useCallback(() => run(api.lightsDevices().then(setDevices)), [run]);
  return (
    <fieldset>
      <legend>{t("lightsOutput")}</legend>
      <div className="row">
        <label className="inline">
          <span>{t("lightsInterface")}</span>
          <Select
            label={t("lightsInterface")}
            value={output.kind}
            options={OUTPUT_KINDS.map(([kind, key]) => [kind, t(key)])}
            onChange={(kind) => onChange(outputOfKind(kind, output))}
          />
        </label>
        {(output.kind === "enttecPro" || output.kind === "openDmx") && (
          <label className="inline">
            <span>{t("lightsPort")}</span>
            <TextBox label={t("lightsPort")} value={output.port} onChange={(port) => onChange({ ...output, port })} />
          </label>
        )}
        {output.kind === "artNet" && (
          <>
            <label className="inline">
              <span>{t("lightsHost")}</span>
              <TextBox label={t("lightsHost")} value={output.host} onChange={(host) => onChange({ ...output, host })} />
            </label>
            <label className="inline">
              <span>{t("lightsUniverse")}</span>
              <NumberBox label={t("lightsUniverse")} value={output.universe} min={0} max={MAX_ARTNET_UNIVERSE} onChange={(universe) => onChange({ ...output, universe })} />
            </label>
          </>
        )}
        {output.kind === "sacn" && (
          <>
            <label className="inline">
              <span>{t("lightsUniverse")}</span>
              <NumberBox
                label={t("lightsUniverse")}
                value={output.universe}
                min={MIN_SACN_UNIVERSE}
                max={MAX_SACN_UNIVERSE}
                onChange={(universe) => onChange({ ...output, universe })}
              />
            </label>
            <label className="inline">
              <span>{t("lightsSacnHost")}</span>
              <TextBox label={t("lightsSacnHost")} value={output.host ?? ""} empty onChange={(host) => onChange({ ...output, host: host || null })} />
            </label>
          </>
        )}
        <button onClick={find}>{t("lightsFind")}</button>
      </div>
      {output.kind === "artNet" && <p className="muted">{t("lightsArtNetHostHint")}</p>}
      {output.kind === "openDmx" && <p className="muted">{t("lightsOpenDmxHint")}</p>}
      {output.kind === "soundSwitch" && <p className="muted">{t("lightsSoundSwitchHint")}</p>}
      {devices && (
        <div>
          <strong>{t("lightsFound")}</strong>
          {devices.outputs.length === 0 ? (
            <p className="muted">{t("lightsNoneFound")}</p>
          ) : (
            <ul className="lights-found">
              {devices.outputs.map((found) => (
                <li key={JSON.stringify(found.choice)}>
                  <span>
                    {t(OUTPUT_KINDS.find(([kind]) => kind === found.choice.kind)?.[1] ?? "lightsOutputNone")} · {found.label}
                  </span>
                  <button disabled={sameOutput(found.choice, output)} onClick={() => onChange(found.choice)}>
                    {t("lightsUse")}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </fieldset>
  );
}

/** A line of text, applied on Enter or when the field is left. Empty text is refused unless `empty`. */
function TextBox({ value, onChange, label, empty = false }: { value: string; onChange: (text: string) => void; label: string; empty?: boolean }) {
  const [text, setText] = useState(value);
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setText(value);
  }
  const apply = () => {
    const next = text.trim();
    if (next === value) return;
    if (next !== "" || empty) onChange(next);
    else setText(value);
  };
  return (
    <input
      aria-label={label}
      value={text}
      maxLength={MAX_LIGHTS_NAME_CHARS * 4}
      onChange={(e) => setText(e.target.value)}
      onBlur={apply}
      onKeyDown={(e) => e.key === "Enter" && apply()}
    />
  );
}

/** The fixtures: name, address and channels, each started from a preset. */
function FixturesSection({ setup, change }: { setup: LightsSetup; change: Change }) {
  const { t } = useApp();
  const [preset, setPreset] = useState(FIXTURE_PRESETS[0].id);
  const clashes = overlapping(setup.fixtures);
  const add = () => {
    const channels = FIXTURE_PRESETS.find((p) => p.id === preset)?.channels ?? [];
    change((s) => addFixture(s, t("lightsNewFixture", { n: s.fixtures.length + 1 }), channels));
  };
  return (
    <fieldset>
      <legend>{t("lightsFixtures")}</legend>
      <p className="muted">{t("lightsFixturesHint")}</p>
      <div className="row">
        <Select label={t("lightsAddFixture")} value={preset} options={FIXTURE_PRESETS.map((p) => [p.id, t(p.label)])} onChange={setPreset} />
        <button disabled={setup.fixtures.length >= MAX_FIXTURES} onClick={add}>
          {t("lightsAddFixture")}
        </button>
      </div>
      <LibraryPicker
        disabled={setup.fixtures.length >= MAX_FIXTURES}
        onAdd={(name, mode) => change((s) => addFixture(s, name.slice(0, MAX_LIGHTS_NAME_CHARS), mode.channels, mode.labels))}
      />
      {setup.fixtures.map((fixture) => (
        <FixtureRow
          key={fixture.id}
          fixture={fixture}
          clash={clashes.has(fixture.id)}
          onChange={(edit) => change((s) => changeFixture(s, fixture.id, edit))}
          onRemove={() => change((s) => removeFixture(s, fixture.id))}
        />
      ))}
    </fieldset>
  );
}

function FixtureRow(props: { fixture: Fixture; clash: boolean; onChange: (edit: Partial<Omit<Fixture, "id">>) => void; onRemove: () => void }) {
  const { fixture, clash, onChange, onRemove } = props;
  const { t } = useApp();
  const last = fixture.address + fixture.channels.length - 1;
  const setChannel = (i: number, channel: Channel) => onChange({ channels: fixture.channels.map((c, j) => (j === i ? channel : c)) });
  return (
    <div className={clash ? "lights-item clash" : "lights-item"}>
      <div className="row">
        <TextBox label={t("lightsName")} value={fixture.name} onChange={(name) => onChange({ name: name.slice(0, MAX_LIGHTS_NAME_CHARS) })} />
        <label className="inline">
          <span>{t("lightsAddress")}</span>
          <NumberBox
            label={t("lightsAddress")}
            value={fixture.address}
            min={1}
            max={UNIVERSE_SIZE - fixture.channels.length + 1}
            onChange={(address) => onChange({ address })}
          />
        </label>
        <span className="muted">
          {t("lightsChannelRange", { from: fixture.address, to: last })}
          {clash && ` · ${t("lightsOverlap")}`}
        </span>
        <button onClick={onRemove}>{t("lightsRemove")}</button>
      </div>
      <div className="row lights-channels">
        {fixture.channels.map((channel, i) => (
          <span key={i} className="inline lights-channel">
            <span className="muted">{fixture.address + i}</span>
            {fixture.labels[i] && <span className="lights-label">{fixture.labels[i]}</span>}
            <Select
              label={`${t("lightsChannels")} ${fixture.address + i}`}
              value={channel.kind}
              options={CHANNEL_KINDS.map((kind) => [kind, t(CHANNEL_KEYS[kind])])}
              onChange={(kind) => setChannel(i, channelOf(kind))}
            />
            {channel.kind === "fixed" && (
              <NumberBox label={t("lightsFixedValue")} value={channel.value} min={0} max={255} onChange={(value) => setChannel(i, { kind: "fixed", value })} />
            )}
            {channel.kind === "dimmerRange" && (
              <>
                <span className="lights-label">{t("lightsDimmerDark")}</span>
                <NumberBox label={t("lightsDimmerDark")} value={channel.dark} min={0} max={255} onChange={(dark) => setChannel(i, { ...channel, dark })} />
                <span className="lights-label">{t("lightsDimmerBright")}</span>
                <NumberBox label={t("lightsDimmerBright")} value={channel.bright} min={0} max={255} onChange={(bright) => setChannel(i, { ...channel, bright })} />
              </>
            )}
            {channel.kind === "shutter" && (
              <>
                <span className="lights-label">{t("lightsShutterOpen")}</span>
                <NumberBox label={t("lightsShutterOpen")} value={channel.open} min={0} max={255} onChange={(open) => setChannel(i, { ...channel, open })} />
                <span className="lights-label">{t("lightsShutterClosed")}</span>
                <NumberBox label={t("lightsShutterClosed")} value={channel.closed} min={0} max={255} onChange={(closed) => setChannel(i, { ...channel, closed })} />
              </>
            )}
          </span>
        ))}
        <button
          title={t("lightsAddChannel")}
          aria-label={t("lightsAddChannel")}
          disabled={fixture.channels.length >= MAX_FIXTURE_CHANNELS || last >= UNIVERSE_SIZE}
          onClick={() => onChange({ channels: [...fixture.channels, { kind: "unused" }] })}
        >
          +
        </button>
        <button
          title={t("lightsRemoveChannel")}
          aria-label={t("lightsRemoveChannel")}
          disabled={fixture.channels.length <= 1}
          onClick={() => onChange({ channels: fixture.channels.slice(0, -1), labels: fixture.labels.slice(0, fixture.channels.length - 1) })}
        >
          −
        </button>
      </div>
    </div>
  );
}

/** How long the library search waits after the last key. */
const SEARCH_AFTER_MS = 250;

/**
 * Adds a fixture from the fixture library: find the model, pick its DMX mode.
 * The library can be brought up to date from the internet, or set back to the built-in one.
 */
function LibraryPicker({ disabled, onAdd }: { disabled: boolean; onAdd: (name: string, mode: LibraryMode) => void }) {
  const { t, lang } = useApp();
  const run = useRun();
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<LibraryMatches | null>(null);
  const [info, setInfo] = useState<LibraryInfo | null>(null);
  const [updating, setUpdating] = useState(false);
  /** How many models the library had before the last update, to say what it changed. */
  const [before, setBefore] = useState<number | null>(null);
  const [model, setModel] = useState<LibraryFixture | null>(null);
  const [mode, setMode] = useState(0);
  useEffect(() => {
    run(api.lightsLibraryInfo().then(setInfo));
  }, [run]);
  const replace = (call: () => Promise<LibraryInfo>, update: boolean) => {
    const models = info?.models ?? null;
    setUpdating(true);
    run(
      call()
        .then((next) => {
          setInfo(next);
          setModel(null);
          setBefore(update ? models : null);
        })
        .finally(() => setUpdating(false)),
    );
  };
  useEffect(() => {
    if (query.trim() === "") return;
    // Searches run side by side: one that ends after the query changed is not shown.
    let current = true;
    const timer = window.setTimeout(
      () =>
        run(
          api.lightsLibrary(query).then((matches) => {
            if (current) setFound(matches);
          }),
        ),
      SEARCH_AFTER_MS,
    );
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
    // A new library searches again.
  }, [query, run, info]);
  const shown = query.trim() === "" ? null : found;
  const chosen = model?.modes[mode];
  const downloaded = info?.source.origin.kind === "downloaded";
  return (
    <div className="lights-library">
      <div className="row">
        <label className="inline">
          <span>{t("lightsLibrary")}</span>
          <input
            type="search"
            aria-label={t("lightsLibrarySearch")}
            placeholder={t("lightsLibrarySearch")}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setModel(null);
            }}
          />
        </label>
        {model && chosen && (
          <>
            <strong>
              {model.manufacturer} {model.name}
            </strong>
            <Select
              label={t("lightsLibraryMode")}
              value={mode}
              options={model.modes.map((m, i): [number, string] => [i, t("lightsLibraryModeName", { name: m.name, n: m.channels.length })])}
              onChange={setMode}
            />
            <button disabled={disabled} onClick={() => onAdd(model.name, chosen)}>
              {t("lightsAddFixture")}
            </button>
          </>
        )}
      </div>
      {chosen && chosen.check.length > 0 && <p className="receiver-state error">{t("lightsLibraryCheck", { labels: chosen.check.join(", ") })}</p>}
      {chosen && !followsLooks(chosen.channels) && <p className="muted">{t("lightsLibraryNoLook")}</p>}
      {shown && !model && (
        <>
          {shown.total === 0 ? (
            <p className="muted">{t("lightsLibraryNone")}</p>
          ) : (
            <ul className="lights-found">
              {shown.fixtures.map((fixture) => (
                <li key={fixture.key}>
                  <button
                    onClick={() => {
                      setModel(fixture);
                      setMode(0);
                    }}
                  >
                    {fixture.manufacturer} {fixture.name}
                  </button>
                  <span className="muted">{t("lightsLibraryModes", { n: fixture.modes.length })}</span>
                </li>
              ))}
            </ul>
          )}
          {shown.total > shown.fixtures.length && <p className="muted">{t("lightsLibraryMore", { shown: shown.fixtures.length, total: shown.total })}</p>}
        </>
      )}
      <p className="muted">{t("lightsLibraryHint")}</p>
      {info && (
        <>
          <div className="row">
            <span className="muted">
              {t(downloaded ? "lightsLibraryDownloaded" : "lightsLibraryBuiltIn", { models: info.models, date: calendarDay(info.source.dateMs, lang) })}
              {before !== null && ` ${t("lightsLibraryUpdated", { before })}`}
            </span>
            <button disabled={updating} title={t("lightsLibraryUpdateHint")} onClick={() => replace(api.lightsLibraryUpdate, true)}>
              {t(updating ? "lightsLibraryUpdating" : "lightsLibraryUpdate")}
            </button>
            {downloaded && (
              <button disabled={updating} onClick={() => replace(api.lightsLibraryReset, false)}>
                {t("lightsLibraryReset")}
              </button>
            )}
          </div>
          <details className="muted lights-licence">
            <summary>{t("lightsLibraryCredit", { source: info.source.name, copyright: info.source.copyright, url: info.source.url })}</summary>
            <pre aria-label={t("lightsLibraryLicence")}>{info.source.licence}</pre>
          </details>
        </>
      )}
    </div>
  );
}

/** The looks: colours, intensity, effect, fade, which fixtures, extra channels; each can be held to try it. */
function LooksSection({ setup, change }: { setup: LightsSetup; change: Change }) {
  const { t } = useApp();
  return (
    <fieldset>
      <legend>{t("lightsLooks")}</legend>
      <p className="muted">{t("lightsLooksHint")}</p>
      {setup.looks.map((look) => (
        <LookRow
          key={look.id}
          look={look}
          fixtures={setup.fixtures}
          onChange={(edit) => change((s) => changeLook(s, look.id, edit))}
          onRemove={() => change((s) => removeLook(s, look.id))}
        />
      ))}
      <button disabled={setup.looks.length >= MAX_LOOKS} onClick={() => change((s) => addLook(s, t("lightsNewLook", { n: s.looks.length + 1 })))}>
        {t("lightsAddLook")}
      </button>
    </fieldset>
  );
}

function LookRow(props: { look: Look; fixtures: Fixture[]; onChange: (edit: Partial<Omit<Look, "id">>) => void; onRemove: () => void }) {
  const { look, fixtures, onChange, onRemove } = props;
  const { t } = useApp();
  const run = useRun();
  const status = useLightsStatus();
  const held = status?.hold === look.id;
  const [channelsText, setChannelsText] = useState(formatChannelValues(look.channels));
  const toggleFixture = (id: number, on: boolean) => onChange({ fixtures: on ? [...look.fixtures, id] : look.fixtures.filter((f) => f !== id) });
  return (
    <div className="lights-item">
      <div className="row">
        <TextBox label={t("lightsName")} value={look.name} onChange={(name) => onChange({ name: name.slice(0, MAX_LIGHTS_NAME_CHARS) })} />
        <button aria-pressed={held} className={held ? "held" : undefined} onClick={() => run(api.lightsHold(held ? null : look.id))}>
          {held ? t("lightsRelease") : t("lightsHoldLook")}
        </button>
        <button onClick={onRemove}>{t("lightsRemove")}</button>
      </div>
      <div className="row">
        <span>{t("lightsColors")}</span>
        {look.colors.map((color, i) => (
          <ColorBox
            key={i}
            label={t("lightsColor", { n: i + 1 })}
            value={color}
            onChange={(next) => next && onChange({ colors: look.colors.map((c, j) => (j === i ? next : c)) })}
          />
        ))}
        <button
          aria-label={t("lightsAddColor")}
          title={t("lightsAddColor")}
          disabled={look.colors.length >= MAX_LOOK_COLORS}
          onClick={() => onChange({ colors: [...look.colors, look.colors[look.colors.length - 1] ?? "#FFFFFF"] })}
        >
          +
        </button>
        <button aria-label={t("lightsRemoveColor")} title={t("lightsRemoveColor")} disabled={look.colors.length <= 1} onClick={() => onChange({ colors: look.colors.slice(0, -1) })}>
          −
        </button>
      </div>
      <div className="row">
        <label className="inline">
          <span>{t("lightsIntensity")}</span>
          <NumberBox label={t("lightsIntensity")} value={look.intensity} min={0} max={100} unit="%" slider onChange={(intensity) => onChange({ intensity })} />
        </label>
        <label className="inline">
          <span>{t("lightsEffect")}</span>
          <Select
            label={t("lightsEffect")}
            value={look.effect}
            options={(Object.keys(EFFECT_KEYS) as LookEffect[]).map((effect) => [effect, t(EFFECT_KEYS[effect])])}
            onChange={(effect) => onChange({ effect })}
          />
        </label>
        {look.effect !== "static" && (
          <label className="inline">
            <span>{t("lightsStep")}</span>
            <NumberBox label={t("lightsStep")} value={look.stepMs} min={MIN_EFFECT_MS} max={MAX_EFFECT_MS} step={50} unit="ms" onChange={(stepMs) => onChange({ stepMs })} />
          </label>
        )}
        <label className="inline">
          <span>{t("lightsFade")}</span>
          <NumberBox label={t("lightsFade")} value={look.fadeMs} min={0} max={MAX_FADE_MS} step={100} unit="ms" onChange={(fadeMs) => onChange({ fadeMs })} />
        </label>
        <label className="inline">
          <span>{t("lightsStrobeValue")}</span>
          <NumberBox label={t("lightsStrobeValue")} value={look.strobe} min={0} max={255} onChange={(strobe) => onChange({ strobe })} />
        </label>
      </div>
      {(look.effect === "strobe" || look.strobe > 0) && <p className="receiver-state error">{t("lightsFlashingWarning")}</p>}
      {fixtures.length > 0 && (
        <div className="row">
          <span>{t("lightsLookFixtures")}</span>
          {fixtures.map((fixture) => (
            <label key={fixture.id} className="inline">
              <input type="checkbox" checked={look.fixtures.includes(fixture.id)} onChange={(e) => toggleFixture(fixture.id, e.target.checked)} />
              {fixture.name}
            </label>
          ))}
          {look.fixtures.length === 0 && <span className="muted">{t("lightsAllFixtures")}</span>}
        </div>
      )}
      <div className="row">
        <label className="inline">
          <span>{t("lightsExtraChannels")}</span>
          <input
            aria-label={t("lightsExtraChannels")}
            className={parseChannelValues(channelsText) ? undefined : "invalid"}
            value={channelsText}
            placeholder="100=255"
            onChange={(e) => setChannelsText(e.target.value)}
            onBlur={() => {
              const channels = parseChannelValues(channelsText);
              if (channels) onChange({ channels });
              else setChannelsText(formatChannelValues(look.channels));
            }}
          />
        </label>
        <span className="muted">{t("lightsExtraChannelsHint")}</span>
      </div>
    </div>
  );
}

/** Which look each stage of the show takes, and which cues flash which look. */
function WhenSection({ setup, change }: { setup: LightsSetup; change: Change }) {
  const { t } = useApp();
  const looks = setup.looks.map((look): [number, string] => [look.id, look.name]);
  return (
    <fieldset>
      <legend>{t("lightsWhen")}</legend>
      <p className="muted">{t("lightsWhenHint")}</p>
      <div className="lights-when">
        {(Object.keys(STAGE_KEYS) as Stage[]).map((stage) => (
          <label key={stage} className="inline">
            <span>{stageText(t, stage)}</span>
            <Select
              label={stageText(t, stage)}
              value={setup.stages[stage] ?? 0}
              options={[[0, t("lightsKeep")], ...looks]}
              onChange={(look) =>
                change((s) => {
                  const stages = { ...s.stages };
                  if (look === 0) delete stages[stage];
                  else stages[stage] = look;
                  return { ...s, stages };
                })
              }
            />
          </label>
        ))}
      </div>
      <h4>{t("lightsFlashes")}</h4>
      <p className="muted">{t("lightsFlashesHint")}</p>
      <div className="lights-when">
        {(Object.keys(CUE_LABEL) as Cue[]).map((cue) => {
          const flash = setup.flashes[cue];
          const setFlash = (next: typeof flash) =>
            change((s) => {
              const flashes = { ...s.flashes };
              if (next) flashes[cue] = next;
              else delete flashes[cue];
              return { ...s, flashes };
            });
          return (
            <label key={cue} className="inline">
              <span>{t(CUE_LABEL[cue])}</span>
              <Select
                label={t(CUE_LABEL[cue])}
                value={flash?.look ?? 0}
                options={[[0, t("lightsNoFlash")], ...looks]}
                onChange={(look) => setFlash(look === 0 ? undefined : { look, ms: flash?.ms ?? 1500 })}
              />
              {flash && (
                <NumberBox
                  label={t("lightsFlashLength")}
                  value={flash.ms}
                  min={MIN_FLASH_MS}
                  max={MAX_FLASH_MS}
                  step={100}
                  unit="ms"
                  onChange={(ms) => setFlash({ ...flash, ms })}
                />
              )}
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

/** Where the show's events go: OSC over UDP and a MIDI output. */
function MessagesSection({ setup, change }: { setup: LightsSetup; change: Change }) {
  const { t } = useApp();
  const run = useRun();
  const [midiPorts, setMidiPorts] = useState<string[] | null>(null);
  const { osc, midi } = setup.messages;
  const setMessages = (edit: Partial<LightsSetup["messages"]>) => change((s) => ({ ...s, messages: { ...s.messages, ...edit } }));
  const findPorts = () => run(api.lightsDevices().then((devices) => setMidiPorts(devices.midiPorts)));
  return (
    <fieldset>
      <legend>{t("lightsMessages")}</legend>
      <p className="muted">{t("lightsMessagesHint")}</p>
      <div className="row">
        <Switch label={t("lightsOsc")} checked={osc !== null} onChange={(on) => setMessages({ osc: on ? { host: "127.0.0.1", port: 7700 } : null })} />
        <span>{t("lightsOsc")}</span>
        {osc && (
          <>
            <TextBox label={t("lightsHost")} value={osc.host} onChange={(host) => setMessages({ osc: { ...osc, host } })} />
            <NumberBox label={t("lightsPort")} value={osc.port} min={1} max={65535} onChange={(port) => setMessages({ osc: { ...osc, port } })} />
          </>
        )}
      </div>
      <div className="row">
        <Switch
          label={t("lightsMidi")}
          checked={midi !== null}
          onChange={(on) => {
            if (on) findPorts();
            setMessages({ midi: on ? { port: midiPorts?.[0] ?? "", channel: 1 } : null });
          }}
        />
        <span>{t("lightsMidi")}</span>
        {midi && (
          <>
            <span onFocus={() => midiPorts === null && findPorts()}>
              <Select
                label={t("lightsMidiPort")}
                value={midi.port}
                options={[...new Set([midi.port, ...(midiPorts ?? [])])].filter((port) => port !== "").map((port): [string, string] => [port, port])}
                onChange={(port) => setMessages({ midi: { ...midi, port } })}
              />
            </span>
            <button onClick={findPorts}>{t("lightsFind")}</button>
            <label className="inline">
              <span>{t("lightsMidiChannel")}</span>
              <NumberBox label={t("lightsMidiChannel")} value={midi.channel} min={1} max={16} onChange={(channel) => setMessages({ midi: { ...midi, channel } })} />
            </label>
          </>
        )}
      </div>
      {midi && midiPorts?.length === 0 && <p className="muted">{t("lightsNoMidiPorts")}</p>}
      {midi && (
        <p className="muted">
          {t("lightsMidiNotes", { stage: FIRST_STAGE_NOTE, cue: FIRST_CUE_NOTE, start: SHOW_STARTED_NOTE, end: SHOW_ENDED_NOTE, buzz: BUZZ_NOTE })}
        </p>
      )}
    </fieldset>
  );
}

/** Each fixture's colour now, and the universe's channels. */
function Preview({ fixtures }: { fixtures: Fixture[] }) {
  const { t } = useApp();
  const [slots, setSlots] = useState<number[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const read = () =>
      api
        .lightsUniverse()
        .then((next) => live && setSlots(next))
        .catch((e) => live && setFailed(errorText(t, e)));
    read();
    const timer = window.setInterval(read, PREVIEW_EVERY_MS);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [t]);
  return (
    <fieldset>
      <legend>{t("lightsPreview")}</legend>
      {failed && <p className="receiver-state error">{failed}</p>}
      <div className="lights-swatches">
        {slots &&
          fixtures.map((fixture) => {
            const color = fixtureColor(fixture, slots);
            return (
              color && (
                <span key={fixture.id} className="lights-swatch" title={fixture.name}>
                  <span style={{ background: color }} />
                  {fixture.name}
                </span>
              )
            );
          })}
      </div>
      <div className="lights-universe" role="img" aria-label={t("lightsPreview")}>
        {slots?.map((value, i) => (
          <span key={i} title={t("lightsChannelValue", { n: i + 1, value })} style={{ opacity: 0.15 + (0.85 * value) / 255 }} />
        ))}
      </div>
    </fieldset>
  );
}
