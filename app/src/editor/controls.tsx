// Small form controls of the editor, in the look of a desktop app.

import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import { parseTrackTime, trackTime } from "../format";
import { mediaFileName } from "../media";
import { shortcutLabel, type Action } from "./actions";

/**
 * True around the control of a property the selected slides have different
 * values for: it shows no value (Mixed), and setting one sets it on all.
 */
export const MixedContext = createContext(false);

export function Switch({ checked, onChange, disabled = false, label }: { checked: boolean; onChange: (on: boolean) => void; disabled?: boolean; label: string }) {
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked && !mixed}
      aria-label={mixed ? `${label}: ${t("mixedValues")}` : label}
      title={mixed ? t("mixedValues") : undefined}
      className={mixed ? "switch mixed" : checked ? "switch on" : "switch"}
      disabled={disabled}
      onClick={() => onChange(mixed || !checked)}
    >
      <span className="switch-knob" />
    </button>
  );
}

/**
 * A button per answer (A, B, C…), pressed for the right ones in `value`.
 * With `keepOne`, the last pressed button cannot be let go; `none` is said
 * while none is pressed. Mixed, none shows pressed and a click makes that
 * answer the only right one.
 */
export function AnswerToggles(props: { label: string; count: number; value: number[]; keepOne: boolean; none: string; onChange: (answers: number[]) => void }) {
  const { label, count, value, keepOne, none, onChange } = props;
  const mixed = useContext(MixedContext);
  const pressed = mixed ? [] : value;
  return (
    <span className="answer-toggles" role="group" aria-label={label}>
      {ANSWER_LETTERS.slice(0, count).map((letter, i) => {
        const on = pressed.includes(i);
        return (
          <button
            key={letter}
            type="button"
            className={on ? "answer-toggle on" : "answer-toggle"}
            aria-pressed={on}
            aria-label={`${label}: ${letter}`}
            disabled={on && keepOne && pressed.length === 1}
            onClick={() => onChange(on ? pressed.filter((answer) => answer !== i) : [...pressed, i])}
          >
            {letter}
          </button>
        );
      })}
      {!mixed && pressed.length === 0 && <span className="answer-toggles-none">{none}</span>}
    </span>
  );
}

/**
 * A number, whole unless `step` is a fraction. The text may be empty or half
 * typed while focused; the value changes whenever the text is a number in
 * [min, max], and leaving the field brings a number outside to its nearest
 * limit (so typing 150 into a box from 25 does not stop at 25 after the 1).
 * With `slider`, a slider over [min, max] comes before the box.
 */
export function NumberBox(props: {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  label: string;
  slider?: boolean;
}) {
  const { value, onChange, min = -Infinity, max = Infinity, step = 1, unit, label, slider = false } = props;
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  const clamp = (n: number) => Math.min(max, Math.max(min, n));
  const shownText = mixed ? "" : String(value);
  const [text, setText] = useState(shownText);
  const [shown, setShown] = useState(shownText);
  if (shownText !== shown) {
    // The value changed from outside (undo, clamping, other slides): show it.
    setShown(shownText);
    setText(shownText);
  }
  return (
    <span className={slider ? "number-box with-slider" : "number-box"}>
      {slider && (
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          aria-label={label}
          value={value}
          className={mixed ? "mixed" : undefined}
          onChange={(e) => onChange(Number(e.target.value))}
        />
      )}
      <input
        type="number"
        inputMode={Number.isInteger(step) ? "numeric" : "decimal"}
        step={step}
        min={Number.isFinite(min) ? min : undefined}
        max={Number.isFinite(max) ? max : undefined}
        aria-label={label}
        placeholder={mixed ? t("mixedValues") : undefined}
        value={text}
        onChange={(e) => {
          const next = e.target.value;
          setText(next);
          const typed = typedNumber(next);
          if (typed !== null && typed === clamp(typed)) onChange(typed);
        }}
        onBlur={() => {
          const typed = typedNumber(text);
          if (typed !== null && (mixed || clamp(typed) !== value)) onChange(clamp(typed));
          setText(shownText);
        }}
      />
      {unit && <span className="unit">{unit}</span>}
    </span>
  );
}

/** The number typed into a field; null for an empty or half-typed one. */
function typedNumber(text: string): number | null {
  const number = Number(text);
  return text.trim() !== "" && Number.isFinite(number) ? number : null;
}

/**
 * A time in a track, typed as `parseTrackTime` reads it. The text may be
 * anything while focused; Enter or leaving the field applies it, and text
 * that is no time in [min, max] goes back to the value.
 */
export function TimeBox(props: { value: number; onChange: (ms: number) => void; min?: number; max?: number; label: string; disabled?: boolean }) {
  const { value, onChange, min = 0, max = Infinity, label, disabled } = props;
  const [text, setText] = useState(trackTime(value));
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setText(trackTime(value));
  }
  const apply = () => {
    const ms = parseTrackTime(text);
    if (ms !== null && ms >= min && ms <= max && ms !== value) onChange(ms);
    else setText(trackTime(value));
  };
  return (
    <input
      className="time-box"
      aria-label={label}
      value={text}
      disabled={disabled}
      onChange={(e) => setText(e.target.value)}
      onBlur={apply}
      onKeyDown={(e) => e.key === "Enter" && apply()}
    />
  );
}

/** The value of the Mixed entry of a select; no option has it. */
const MIXED_OPTION = "\u0000mixed";

export function Select<T extends string | number>(props: { value: T; options: [T, string][]; onChange: (value: T) => void; label: string; disabled?: boolean }) {
  const { value, options, onChange, label, disabled } = props;
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  return (
    <select
      aria-label={label}
      value={mixed ? MIXED_OPTION : String(value)}
      disabled={disabled}
      onChange={(e) => {
        const picked = options.find(([option]) => String(option) === e.target.value);
        if (picked) onChange(picked[0]);
      }}
    >
      {mixed && (
        <option value={MIXED_OPTION} disabled>
          {t("mixedValues")}
        </option>
      )}
      {options.map(([option, text]) => (
        <option key={String(option)} value={String(option)}>
          {text}
        </option>
      ))}
    </select>
  );
}

/** A media file of the slide: its name, a button to choose another, one to remove it. */
export function MediaPicker({ file, onChoose, onRemove, label }: { file: string | null; onChoose: () => void; onRemove: () => void; label: string }) {
  const { t } = useApp();
  const mixed = useContext(MixedContext);
  return (
    <span className="media-picker">
      <span className={mixed ? "media-name mixed" : "media-name"} title={mixed ? undefined : (file ?? undefined)}>
        {mixed ? t("mixedValues") : file ? mediaFileName(file) : t("noFile")}
      </span>
      <button type="button" className="mini" aria-label={`${label}: ${t("chooseFile")}`} title={t("chooseFile")} onClick={onChoose}>
        …
      </button>
      {(file || mixed) && (
        <button type="button" className="mini" aria-label={`${label}: ${t("remove")}`} title={t("remove")} onClick={onRemove}>
          ×
        </button>
      )}
    </span>
  );
}

/** A dialog over the window; Escape or the close button cancels. */
export function Dialog(props: { title: string; onClose: () => void; children: ReactNode; footer: ReactNode; className?: string }) {
  const { title, onClose, children, footer, className } = props;
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Focus the control marked for it (a footer button too), or else the first field, so typing starts at once.
    const body = box.current?.querySelector(".dialog-body");
    (box.current?.querySelector<HTMLElement>("[data-autofocus]") ?? body?.querySelector<HTMLElement>("textarea, input, select, button"))?.focus();
  }, []);
  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={box}
        className={className ? `dialog ${className}` : "dialog"}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onClose();
          }
        }}
      >
        <header className="dialog-title">
          <span>{title}</span>
          <button type="button" className="dialog-close" aria-label="×" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="dialog-body">{children}</div>
        <footer className="dialog-footer">{footer}</footer>
      </div>
    </div>
  );
}

/**
 * A box over the window at a point, kept inside the window also as it grows
 * (a picker folding a part open); closes on any click outside or Escape.
 */
export function Popup(props: { x: number; y: number; onClose: () => void; className: string; role: string; label?: string; children: ReactNode }) {
  const { x, y, onClose, className, role, label, children } = props;
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const element = box.current;
    if (!element) return;
    const place = () => {
      const rect = element.getBoundingClientRect();
      element.style.left = `${Math.max(0, Math.min(x, window.innerWidth - rect.width - 4))}px`;
      element.style.top = `${Math.max(0, Math.min(y, window.innerHeight - rect.height - 4))}px`;
    };
    place();
    element.focus();
    const resized = new ResizeObserver(place);
    resized.observe(element);
    return () => resized.disconnect();
  }, [x, y]);
  return (
    <div
      className="popup-backdrop"
      onMouseDown={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div
        ref={box}
        className={className}
        role={role}
        aria-label={label}
        tabIndex={-1}
        style={{ left: x, top: y }}
        onMouseDown={(e) => e.stopPropagation()}
        onContextMenu={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          // Only the popup closes, not what the editor's Esc does.
          e.stopPropagation();
          onClose();
        }}
      >
        {children}
      </div>
    </div>
  );
}

/** A context menu of actions at a point of the window. */
export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: (Action | "separator")[]; onClose: () => void }) {
  return (
    <Popup x={x} y={y} onClose={onClose} className="context-menu" role="menu">
      {items.map((item, i) =>
        item === "separator" ? (
          <div key={i} className="menu-separator" />
        ) : (
          <button
            key={i}
            type="button"
            role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
            aria-checked={item.checked}
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.run();
            }}
          >
            <span className="menu-icon">{item.checked ? "✓" : item.icon}</span>
            <span>{item.label}</span>
            {item.keys && <small>{shortcutLabel(item)}</small>}
          </button>
        ),
      )}
    </Popup>
  );
}
