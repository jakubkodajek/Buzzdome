// A gradient property: a preview of the gradient, and the picker of
// ready-made gradients and the user's own it opens (spec QZ-1).

import { useContext, useState } from "react";
import { useApp } from "../app-context";
import { PaletteIcon } from "../icons";
import { useStore } from "../store";
import { gradientCss } from "../theme/slide-style";
import type { Gradient } from "../types";
import { ContextMenu, MixedContext, Popup } from "./controls";
import { GRADIENT_PRESETS, myGradients, removeMyGradient, sameGradient } from "./gradients";

/** The gradient property's control; no gradient leaves the theme's look. */
export function GradientBox({ label, value, onChange }: { label: string; value: Gradient | null; onChange: (gradient: Gradient | null) => void }) {
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  const [open, setOpen] = useState<{ x: number; y: number } | null>(null);
  const shown = mixed ? null : value;
  const openAt = (element: HTMLElement) => {
    const box = element.getBoundingClientRect();
    setOpen({ x: box.left, y: box.bottom + 2 });
  };
  return (
    <span className="gradient-box">
      <button
        type="button"
        className={shown ? "gradient-preview" : "gradient-preview none"}
        style={shown ? { background: gradientCss(shown) } : undefined}
        aria-label={`${label}: ${t("choosePreset")}`}
        title={shown ? `${shown.from} → ${shown.to}` : undefined}
        onClick={(e) => openAt(e.currentTarget)}
      >
        {!shown && (mixed ? t("mixedValues") : t("noGradient"))}
      </button>
      <button type="button" className="mini" aria-label={t("choosePreset")} title={t("choosePreset")} aria-haspopup="dialog" onClick={(e) => openAt(e.currentTarget)}>
        <PaletteIcon />
      </button>
      {open && (
        <Popup x={open.x} y={open.y} onClose={() => setOpen(null)} className="gradient-picker" role="dialog" label={label}>
          <GradientPicker
            value={shown}
            onPick={(gradient) => {
              setOpen(null);
              onChange(gradient);
            }}
          />
        </Popup>
      )}
    </span>
  );
}

function GradientPicker({ value, onPick }: { value: Gradient | null; onPick: (gradient: Gradient | null) => void }) {
  const { t } = useApp();
  const mine = useStore(myGradients);
  const [menu, setMenu] = useState<{ x: number; y: number; gradient: Gradient } | null>(null);
  const tile = (gradient: Gradient, own: boolean) => (
    <button
      key={JSON.stringify(gradient)}
      type="button"
      className={value && sameGradient(gradient, value) ? "gradient-tile selected" : "gradient-tile"}
      style={{ background: gradientCss(gradient) }}
      title={`${gradient.from} → ${gradient.to}`}
      aria-label={`${gradient.from} → ${gradient.to}`}
      onClick={() => onPick(gradient)}
      onContextMenu={(e) => {
        if (!own) return;
        e.preventDefault();
        e.stopPropagation();
        setMenu({ x: e.clientX, y: e.clientY, gradient });
      }}
    />
  );
  return (
    <div className="gradient-picker-body">
      <button type="button" className={value === null ? "color-none selected" : "color-none"} onClick={() => onPick(null)}>
        <span className="swatch-color none" />
        {t("noGradient")}
      </button>
      <small className="color-heading">{t("gradientPresets")}</small>
      <div className="gradient-grid">{GRADIENT_PRESETS.map((gradient) => tile(gradient, false))}</div>
      <small className="color-heading">{t("myGradients")}</small>
      {mine.length > 0 ? <div className="gradient-grid">{mine.map((gradient) => tile(gradient, true))}</div> : <p className="gradient-empty">{t("myGradientsHint")}</p>}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={[{ label: t("removeFromMyGradients"), run: () => removeMyGradient(menu.gradient) }]}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
