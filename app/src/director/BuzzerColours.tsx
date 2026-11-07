import { Fragment, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { confirmAction } from "../confirm";
import { CustomColor, Swatch } from "../editor/ColorPicker";
import { Popup } from "../editor/controls";
import type { StringKey } from "../i18n";
import { colourClashText, errorNotice, lightText, type Notice } from "../messages";
import { NoticeBox } from "../notice";
import { sameColor } from "../theme/color";
import type { Light, ReceiverInfo, Rgb } from "../types";

/** The lights in the order a question shows them. */
const LIGHTS: readonly Light[] = ["ready", "answered", "waiting", "first", "locked", "correct", "wrong", "off"];

const LIGHT_HINTS: Record<Light, StringKey> = {
  off: "lightOffHint",
  ready: "lightReadyHint",
  answered: "lightAnsweredHint",
  waiting: "lightWaitingHint",
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

/**
 * Each light's colour on the buzzers, for every quiz (`LightColours`), with
 * the shade the buzzers show of it, and the colours the quizmaster keeps at
 * hand for picking.
 */
export function BuzzerColours({ info, reload }: { info: ReceiverInfo; reload: () => void }) {
  const { t } = useApp();
  const run = useRun();
  // Why the last colour chosen was refused, shown in its light's row.
  const [error, setError] = useState<{ light: Light; notice: Notice } | null>(null);
  const reset = () =>
    run(confirmAction(t, t("buzzerColoursResetConfirm"), t("buzzerColoursReset")).then((yes) => (yes ? api.receiverResetColours().then(() => setError(null)).finally(reload) : undefined)));
  const choose = (light: Light, colour: Rgb) => {
    if (sameColor(colour, info.colours[light])) return;
    api
      .receiverSetColours({ ...info.colours, [light]: colour })
      .then(
        () => setError(null),
        (e: unknown) => setError({ light, notice: errorNotice(t, e) }),
      )
      .finally(reload);
  };
  return (
    <fieldset className="buzzer-colours">
      <legend>{t("buzzerColours")}</legend>
      <p className="muted">{t("buzzerColoursHint")}</p>
      {info.coloursRefused && <NoticeBox notice={{ text: t("buzzerColoursRefusedKept", { problem: colourClashText(t, info.coloursRefused) }) }} />}
      <table className="grid">
        <thead>
          <tr>
            <th scope="col">{t("buzzerColourState")}</th>
            <th scope="col">{t("buzzerColour")}</th>
            <th scope="col">{t("buzzerShade")}</th>
          </tr>
        </thead>
        <tbody>
          {LIGHTS.map((light) => (
            <Fragment key={light}>
              <tr>
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
                <td className="buzzer-shade">
                  <Swatch color={info.shownColours[light]} />
                </td>
              </tr>
              {error?.light === light && (
                <tr>
                  <td colSpan={3}>
                    <NoticeBox notice={error.notice} />
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      <div className="row">
        <span className="spacer" />
        <button onClick={reset}>{t("buzzerColoursReset")}</button>
      </div>
    </fieldset>
  );
}

/**
 * A light's colour: a swatch with its hex code that opens a picker with the
 * most used colours, the quizmaster's own, and any colour. A colour from a
 * row is chosen at once; a custom one once the picker closes, unless Esc
 * closed it.
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
  const kept = mine.some((colour) => sameColor(colour, shown));
  // Set by an Esc on its way to the picker, which closes it on Esc as on a click outside.
  const escaped = useRef(false);
  useEffect(() => {
    if (!open) return;
    escaped.current = false;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") escaped.current = true;
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open]);
  const close = (colour: Rgb | null) => {
    setOpen(null);
    setDraft(null);
    if (colour !== null) onChoose(colour);
  };
  const tile = (colour: Rgb, name: string) => (
    <button
      key={colour}
      type="button"
      className={sameColor(colour, shown) ? "color-tile selected" : "color-tile"}
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
        <Popup x={open.x} y={open.y} onClose={() => close(escaped.current ? null : shown)} className="color-picker" role="dialog" label={label}>
          <div className="color-picker-body">
            <small className="color-heading">{t("coloursCommon")}</small>
            <div className="color-grid">{COMMON_COLOURS.map(({ colour, name }) => tile(colour, t(name)))}</div>
            <small className="color-heading">{t("coloursMine")}</small>
            {mine.length > 0 ? <div className="color-grid">{mine.map((colour) => tile(colour, colour))}</div> : <small className="muted">{t("coloursMineNone")}</small>}
            <button
              type="button"
              className="mini"
              disabled={!kept && full}
              onClick={() => onKeep(kept ? mine.filter((colour) => !sameColor(colour, shown)) : [...mine, shown])}
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
