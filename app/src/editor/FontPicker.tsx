// A text's font property (spec QZ-1): a list that previews the installed
// fonts, the size as a share of the theme's, and bold, italic and underline.

import { useContext, useEffect, useRef, useState } from "react";
import { MAX_TEXT_SIZE, MIN_TEXT_SIZE } from "../bindings/Constants";
import { useApp } from "../app-context";
import { MixedContext, NumberBox, Popup } from "./controls";
import { useInstalledFonts } from "./fonts";
import type { FontValue } from "./property-rows";

/** The theme's size of a text, which `size` is a share of. */
const THEME_SIZE = 100;

/**
 * The font property's control. What the value leaves out is the theme's:
 * its font, its size, and bold where the theme draws the text bold
 * (`themeBold`). A change sets only what differs from the theme.
 */
export function FontBox(props: { label: string; value: FontValue; themeBold: boolean; onChange: (change: Partial<FontValue>) => void }) {
  const { label, value, themeBold, onChange } = props;
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  const [open, setOpen] = useState<{ x: number; y: number; width: number } | null>(null);
  const bold = value.bold ?? themeBold;
  const toggle = (name: "bold" | "italic" | "underline", on: boolean, theme: boolean) => (
    <button
      type="button"
      className={!mixed && on ? "font-toggle on" : "font-toggle"}
      aria-pressed={!mixed && on}
      aria-label={`${label}: ${t(name)}`}
      title={t(name)}
      onClick={() => onChange({ [name]: mixed || !on ? (theme ? undefined : true) : theme ? false : undefined })}
    >
      {t(`${name}Mark`)}
    </button>
  );
  return (
    <span className="font-box">
      <button
        type="button"
        className="font-name"
        aria-haspopup="listbox"
        aria-label={`${label}: ${t("chooseFont")}`}
        style={!mixed && value.font ? { fontFamily: `"${value.font}"` } : undefined}
        onClick={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          setOpen({ x: box.left, y: box.bottom + 2, width: box.width });
        }}
      >
        {mixed ? t("mixedValues") : (value.font ?? t("themeFont"))}
      </button>
      <span className="font-row">
        <NumberBox
          label={`${label}: ${t("fontSize")}`}
          value={value.size ?? THEME_SIZE}
          min={MIN_TEXT_SIZE}
          max={MAX_TEXT_SIZE}
          step={5}
          unit="%"
          onChange={(size) => onChange({ size: size === THEME_SIZE ? undefined : size })}
        />
        {toggle("bold", bold, themeBold)}
        {toggle("italic", value.italic ?? false, false)}
        {toggle("underline", value.underline ?? false, false)}
      </span>
      {open && (
        <Popup x={open.x} y={open.y} onClose={() => setOpen(null)} className="font-picker" role="dialog" label={label}>
          <FontList
            value={mixed ? undefined : value.font}
            width={open.width}
            onPick={(font) => {
              setOpen(null);
              onChange({ font });
            }}
          />
        </Popup>
      )}
    </span>
  );
}

/** The installed fonts, each in its own look, with a search; the first entry is the theme's font. */
function FontList({ value, width, onPick }: { value: string | undefined; width: number; onPick: (font: string | undefined) => void }) {
  const { t } = useApp();
  const fonts = useInstalledFonts();
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement>(null);
  // After the popup has taken the focus, so typing searches at once.
  useEffect(() => search.current?.focus(), []);
  const needle = query.trim().toLocaleLowerCase();
  const shown = needle ? fonts.filter((font) => font.toLocaleLowerCase().includes(needle)) : fonts;
  // A font from another computer stays offered while it is the value.
  const listed = value && !fonts.includes(value) && !needle ? [value, ...shown] : shown;
  return (
    <div className="font-list" style={{ minWidth: width }}>
      <input
        ref={search}
        type="search"
        placeholder={t("searchFonts")}
        aria-label={t("searchFonts")}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && listed.length > 0) onPick(listed[0]);
        }}
      />
      <div className="font-options" role="listbox" aria-label={t("chooseFont")}>
        {!needle && (
          <button type="button" role="option" aria-selected={value === undefined} className={value === undefined ? "selected" : ""} onClick={() => onPick(undefined)}>
            {t("themeFont")}
          </button>
        )}
        {listed.map((font) => (
          <button
            key={font}
            type="button"
            role="option"
            aria-selected={font === value}
            className={font === value ? "selected" : ""}
            style={{ fontFamily: `"${font}"` }}
            onClick={() => onPick(font)}
          >
            {font}
          </button>
        ))}
        {needle && listed.length === 0 && <p className="font-empty">{t("noFontMatch")}</p>}
        {!needle && fonts.length === 0 && <p className="font-empty">{t("loadingFonts")}</p>}
      </div>
    </div>
  );
}
