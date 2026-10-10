// A colour property: a swatch with its hex code, and the colour picker it
// opens (spec QZ-1): the theme's colour, a palette of shades, standard and
// recent colours, and a custom colour with a pipette where the web view has
// one.

import { useContext, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { useApp } from "../app-context";
import { useLatest } from "../hooks";
import { cssColor, formatColor, fromHsv, parseColor, readColor, toHsv, type Hsv } from "../theme/color";
import { useStore } from "../store";
import type { Color } from "../types";
import { MixedContext, Popup } from "./controls";
import { recentColors, rememberColor, SHADES, STANDARD_COLORS } from "./palette";

/** The colour property's control. `none` names the empty value (the theme's colour); without it there is none. */
export function ColorBox(props: { label: string; value: Color | null; none?: string; onChange: (color: Color | null) => void }) {
  const { label, value, none, onChange } = props;
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  const [open, setOpen] = useState<{ x: number; y: number } | null>(null);
  const shown = mixed ? "" : (value ?? "");
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
        <Swatch color={mixed ? null : value} />
      </button>
      <HexField label={label} value={shown} placeholder={mixed ? t("mixedValues") : (none ?? "")} onChange={onChange} />
      {open && (
        <Popup x={open.x} y={open.y} onClose={() => setOpen(null)} className="color-picker" role="dialog" label={label}>
          <ColorPicker
            value={mixed ? null : value}
            none={none}
            onPick={(color) => {
              setOpen(null);
              onChange(color);
            }}
            onChange={onChange}
          />
        </Popup>
      )}
    </span>
  );
}

/** A colour over a chequerboard, so its transparency shows; struck through for none. */
function Swatch({ color }: { color: Color | null }) {
  const css = color && cssColor(color);
  return <span className={color ? "swatch-color" : "swatch-color none"} style={css ? { backgroundImage: `linear-gradient(${css}, ${css}), var(--chequer)` } : undefined} />;
}

/** The hex code, typed or pasted; Enter or leaving the field applies it, text that is no colour goes back. */
function HexField({ label, value, placeholder, onChange }: { label: string; value: string; placeholder: string; onChange: (color: Color) => void }) {
  const { t } = useApp();
  const [text, setText] = useState(value);
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setText(value);
  }
  const apply = (typed: string) => {
    const color = readColor(typed);
    if (color && color !== value) {
      rememberColor(color);
      onChange(color);
    }
    setText(color ?? value);
  };
  return (
    <input
      className="hex-field"
      aria-label={`${label}: ${t("colorCode")}`}
      spellCheck={false}
      placeholder={placeholder}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => apply(text)}
      onKeyDown={(e) => {
        if (e.key === "Enter") apply(text);
      }}
      onPaste={(e) => {
        // A colour pasted from another program applies at once.
        const pasted = e.clipboardData.getData("text");
        if (!readColor(pasted)) return;
        e.preventDefault();
        apply(pasted);
      }}
    />
  );
}

/** The web view's screen pipette (Chromium has one; other web views do not). */
interface EyeDropperApi {
  open: () => Promise<{ sRGBHex: string }>;
}
const EyeDropper = (globalThis as { EyeDropper?: new () => EyeDropperApi }).EyeDropper;

/**
 * The picker's contents. A colour from the palettes is picked and closes
 * it; a custom colour changes as it is dragged (`onChange`) and is
 * remembered among the recent colours once let go.
 */
function ColorPicker(props: { value: Color | null; none?: string; onPick: (color: Color | null) => void; onChange: (color: Color) => void }) {
  const { value, none, onPick, onChange } = props;
  const { t } = useApp();
  const recent = useStore(recentColors);
  const [custom, setCustom] = useState(false);
  const pick = (color: Color) => {
    rememberColor(color);
    onPick(color);
  };
  const tile = (color: Color) => (
    <button
      key={color}
      type="button"
      className={color === value ? "color-tile selected" : "color-tile"}
      title={color}
      aria-label={color}
      style={{ background: cssColor(color) }}
      onClick={() => pick(color)}
    />
  );
  const pipette = async () => {
    if (!EyeDropper) return;
    try {
      const { sRGBHex } = await new EyeDropper().open();
      const picked = parseColor(readColor(sRGBHex) ?? "");
      // The screen has no transparency: the colour keeps the opacity it had.
      if (picked) pick(formatColor({ ...picked, a: parseColor(value ?? "")?.a ?? 1 }));
    } catch {
      // Cancelled with Esc.
    }
  };
  return (
    <div className="color-picker-body">
      {none !== undefined && (
        <button type="button" className={value === null ? "color-none selected" : "color-none"} onClick={() => onPick(null)}>
          <Swatch color={null} />
          {none}
        </button>
      )}
      <div className="color-grid shades">{SHADES.flat().map(tile)}</div>
      <small className="color-heading">{t("standardColors")}</small>
      <div className="color-grid">{STANDARD_COLORS.map(tile)}</div>
      {recent.length > 0 && (
        <>
          <small className="color-heading">{t("recentColors")}</small>
          <div className="color-grid">{recent.map(tile)}</div>
        </>
      )}
      <div className="color-custom-title">
        <button type="button" className="props-section-title" aria-expanded={custom} onClick={() => setCustom(!custom)}>
          <span className={custom ? "fold open" : "fold"}>›</span>
          {t("customColor")}
        </button>
        {EyeDropper && (
          <button type="button" className="mini" title={t("pickFromScreen")} aria-label={t("pickFromScreen")} onClick={() => void pipette()}>
            ⌖
          </button>
        )}
      </div>
      {custom && <CustomColor value={value ?? "#FFFFFF"} onChange={onChange} />}
    </div>
  );
}

/** The saturation and brightness square, the hue and opacity bars, and the hex code. */
function CustomColor({ value, onChange }: { value: Color; onChange: (color: Color) => void }) {
  const { t } = useApp();
  const parsed = parseColor(value) ?? { r: 255, g: 255, b: 255, a: 1 };
  // Kept apart from the value, so a grey or black keeps the hue it was given.
  const [hsv, setHsv] = useState<Hsv>(() => toHsv(parsed));
  const [alpha, setAlpha] = useState(parsed.a);
  const [last, setLast] = useState(value);
  if (value !== last) {
    // Changed from outside (the hex code, undo): follow it.
    const next = parseColor(value) ?? parsed;
    const nextHsv = toHsv(next);
    setLast(value);
    setHsv(nextHsv.s === 0 || nextHsv.v === 0 ? { ...nextHsv, h: hsv.h } : nextHsv);
    setAlpha(next.a);
  }
  const set = (nextHsv: Hsv, nextAlpha: number) => {
    const color = formatColor(fromHsv(nextHsv, nextAlpha));
    setHsv(nextHsv);
    setAlpha(nextAlpha);
    setLast(color);
    if (color !== value) onChange(color);
  };
  const solid = formatColor(fromHsv(hsv));
  const pure = formatColor(fromHsv({ h: hsv.h, s: 1, v: 1 }));
  return (
    <div className="color-custom">
      <Area
        className="color-square"
        label={t("saturationBrightness")}
        style={{ background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, ${pure})` }}
        x={hsv.s}
        y={1 - hsv.v}
        onMove={(x, y) => set({ ...hsv, s: x, v: 1 - y }, alpha)}
        onDone={() => rememberColor(value)}
      />
      <Area
        className="color-bar hue"
        label={t("hue")}
        x={hsv.h / 360}
        onMove={(x) => set({ ...hsv, h: Math.min(359.9, x * 360) }, alpha)}
        onDone={() => rememberColor(value)}
      />
      <Area
        className="color-bar opacity"
        label={t("opacity")}
        style={{ backgroundImage: `linear-gradient(to right, transparent, ${solid}), var(--chequer)` }}
        x={alpha}
        onMove={(x) => set(hsv, Math.round(x * 100) / 100)}
        onDone={() => rememberColor(value)}
      />
      <div className="color-custom-row">
        <Swatch color={value} />
        <HexField label={t("customColor")} value={value} placeholder="" onChange={onChange} />
      </div>
    </div>
  );
}

/**
 * A square or a bar to drag in, with a marker at (`x`, `y`) as fractions of
 * it (a bar has no `y`). Arrow keys move the marker by 1 % (Shift: 10 %).
 */
function Area(props: {
  className: string;
  label: string;
  style?: CSSProperties;
  x: number;
  y?: number;
  onMove: (x: number, y: number) => void;
  onDone: () => void;
}) {
  const { className, label, style, x, y, onMove, onDone } = props;
  // A drag calls the handlers of the latest render, which know the colour as it is by then.
  const latest = useLatest({ onMove, onDone });
  const clamp = (n: number) => Math.min(1, Math.max(0, n));
  const moveTo = (e: PointerEvent | ReactPointerEvent, area: HTMLElement) => {
    const box = area.getBoundingClientRect();
    latest.current.onMove(clamp((e.clientX - box.left) / box.width), clamp((e.clientY - box.top) / box.height));
  };
  const start = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const area = e.currentTarget;
    area.setPointerCapture(e.pointerId);
    area.focus();
    moveTo(e, area);
    const move = (event: PointerEvent) => moveTo(event, area);
    const end = () => {
      area.removeEventListener("pointermove", move);
      area.removeEventListener("pointerup", end);
      area.removeEventListener("pointercancel", end);
      latest.current.onDone();
    };
    area.addEventListener("pointermove", move);
    area.addEventListener("pointerup", end);
    area.addEventListener("pointercancel", end);
  };
  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 0.1 : 0.01;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const move = moves[e.key];
    if (!move || (y === undefined && move[0] === 0)) return;
    e.preventDefault();
    e.stopPropagation();
    onMove(clamp(x + move[0]), clamp((y ?? 0) + move[1]));
  };
  return (
    <div
      className={className}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuenow={Math.round(x * 100)}
      aria-valuemin={0}
      aria-valuemax={100}
      style={style}
      onPointerDown={start}
      onKeyDown={keys}
      onKeyUp={(e) => e.key.startsWith("Arrow") && onDone()}
    >
      <span className="color-marker" style={{ left: `${x * 100}%`, top: y === undefined ? "50%" : `${y * 100}%` }} />
    </div>
  );
}
