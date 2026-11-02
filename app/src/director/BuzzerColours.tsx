import { useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { CustomColor, Swatch } from "../editor/ColorPicker";
import { Popup } from "../editor/controls";
import type { StringKey } from "../i18n";
import { colourClashText, errorNotice, lightText, type Notice } from "../messages";
import { NoticeBox } from "../notice";
import type { Light, ReceiverInfo, Rgb } from "../types";

/** The lights in the order a question shows them. */
const LIGHTS: readonly Light[] = ["ready", "answered", "first", "locked", "correct", "wrong", "off"];

const LIGHT_HINTS: Record<Light, StringKey> = {
  off: "lightOffHint",
  ready: "lightReadyHint",
  answered: "lightAnsweredHint",
  locked: "lightLockedHint",
  first: "lightFirstHint",
  correct: "lightCorrectHint",
  wrong: "lightWrongHint",
};

/**
 * The colours offered first: each one the buzzers show as it is
 * (`colours::Shade`), black for off and dim white as the defaults have it.
 */
const COMMON_COLOURS: readonly { colour: Rgb; name: StringKey }[] = [
  { colour: "#000000", name: "colorBlack" },
  { colour: "#2a2a2a", name: "colorDimWhite" },
  { colour: "#ffffff", name: "colorWhite" },
  { colour: "#ff0000", name: "colorRed" },
  { colour: "#ffaa00", name: "colorOrange" },
  { colour: "#ffff00", name: "colorYellow" },
  { colour: "#00ff00", name: "colorGreen" },
  { colour: "#00ffff", name: "colorTeal" },
  { colour: "#0000ff", name: "colorBlue" },
  { colour: "#aa00ff", name: "colorPurple" },
];

/** Whether two colours are the same, whatever the case of their hex digits. */
const sameColour = (a: Rgb, b: Rgb) => a.toLowerCase() === b.toLowerCase();

/**
 * Each light's colour on the buzzers, for every quiz (`LightColours`), with
 * the shade the buzzers show of it, and the colours the quizmaster keeps at
 * hand for picking.
 */
export function BuzzerColours({ info, reload }: { info: ReceiverInfo; reload: () => void }) {
  const { t } = useApp();
  const run = useRun();
  const [error, setError] = useState<Notice | null>(null);
  const choose = (light: Light, colour: Rgb) => {
    if (sameColour(colour, info.colours[light])) return;
    api
      .receiverSetColours({ ...info.colours, [light]: colour })
      .then(
        () => setError(null),
        (e: unknown) => setError(errorNotice(t, e)),
      )
      .finally(reload);
  };
  return (
    <fieldset className="buzzer-colours">
      <legend>{t("buzzerColours")}</legend>
      <p className="muted">{t("buzzerColoursHint")}</p>
      {info.coloursRefused && <NoticeBox notice={{ text: t("buzzerColoursRefusedKept", { problem: colourClashText(t, info.coloursRefused) }) }} />}
      <table className="grid">
        <tbody>
          {LIGHTS.map((light) => (
            <tr key={light}>
              <th scope="row">
                {lightText(t, light)}
                <small className="muted">{t(LIGHT_HINTS[light])}</small>
              </th>
              <td>
                <BuzzerColourBox
                  label={lightText(t, light)}
                  value={info.colours[light]}
                  mine={info.myColours}
                  full={info.myColours.length >= info.maxMyColours}
                  onChoose={(colour) => choose(light, colour)}
                  onKeep={(mine) => run(api.receiverSetMyColours(mine).then(reload))}
                />
              </td>
              <td className="buzzer-shade" title={t("buzzerShade")}>
                ≈ <Swatch color={info.shownColours[light]} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {error && <NoticeBox notice={error} />}
      <div className="row">
        <span className="spacer" />
        <button onClick={() => run(api.receiverResetColours().then(() => setError(null)).finally(reload))}>{t("buzzerColoursReset")}</button>
      </div>
    </fieldset>
  );
}

/**
 * A light's colour: a swatch with its hex code that opens a picker with the
 * most used colours, the quizmaster's own, and any colour. A colour from a
 * row is chosen at once; a custom one once the picker closes.
 */
function BuzzerColourBox(props: {
  label: string;
  value: Rgb;
  mine: readonly Rgb[];
  /** No more colours can be kept at hand. */
  full: boolean;
  onChoose: (colour: Rgb) => void;
  onKeep: (mine: Rgb[]) => void;
}) {
  const { label, value, mine, full, onChoose, onKeep } = props;
  const { t } = useApp();
  const [open, setOpen] = useState<{ x: number; y: number } | null>(null);
  const [draft, setDraft] = useState<Rgb | null>(null);
  const shown = draft ?? value;
  const kept = mine.some((colour) => sameColour(colour, shown));
  const close = (colour: Rgb) => {
    setOpen(null);
    setDraft(null);
    onChoose(colour);
  };
  const tile = (colour: Rgb, name: string) => (
    <button
      key={colour}
      type="button"
      className={sameColour(colour, shown) ? "color-tile selected" : "color-tile"}
      title={name}
      aria-label={name}
      style={{ background: colour }}
      onClick={() => close(colour)}
    />
  );
  return (
    <span className="color-box">
      <button
        type="button"
        className="swatch"
        aria-label={`${label}: ${t("chooseColor")}`}
        title={t("chooseColor")}
        aria-haspopup="dialog"
        onClick={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          setOpen({ x: box.left, y: box.bottom + 2 });
        }}
      >
        <Swatch color={shown} />
      </button>
      <code>{shown.toLowerCase()}</code>
      {open && (
        <Popup x={open.x} y={open.y} onClose={() => close(shown)} className="color-picker" role="dialog" label={label}>
          <div className="color-picker-body">
            <small className="color-heading">{t("coloursCommon")}</small>
            <div className="color-grid">{COMMON_COLOURS.map(({ colour, name }) => tile(colour, t(name)))}</div>
            <small className="color-heading">{t("coloursMine")}</small>
            {mine.length > 0 ? <div className="color-grid">{mine.map((colour) => tile(colour, colour))}</div> : <small className="muted">{t("coloursMineNone")}</small>}
            <button
              type="button"
              className="mini"
              disabled={!kept && full}
              onClick={() => onKeep(kept ? mine.filter((colour) => !sameColour(colour, shown)) : [...mine, shown])}
            >
              {t(kept ? "coloursForget" : "coloursKeep")}
            </button>
            <small className="color-heading">{t("customColor")}</small>
            {/* A typed code may carry an opacity, which the buzzers have none of. */}
            <CustomColor value={shown} opaque onChange={(colour) => setDraft(colour.slice(0, 7))} />
          </div>
        </Popup>
      )}
    </span>
  );
}
