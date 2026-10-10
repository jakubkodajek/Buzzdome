// Small form controls of the editor, in the look of a desktop app.

import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useApp } from "../app-context";
import { parseTrackTime, trackTime } from "../format";
import { mediaFileName } from "../media";
import { shortcutLabel, type Action } from "./actions";

export function Switch({ checked, onChange, disabled = false, label }: { checked: boolean; onChange: (on: boolean) => void; disabled?: boolean; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={checked ? "switch on" : "switch"}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="switch-knob" />
    </button>
  );
}

/**
 * A number, whole unless `step` is a fraction. The text may be empty or half
 * typed while focused; the value changes whenever the text is a number.
 */
export function NumberBox(props: { value: number; onChange: (value: number) => void; min?: number; max?: number; step?: number; unit?: string; label: string }) {
  const { value, onChange, min, max, step = 1, unit, label } = props;
  const [text, setText] = useState(String(value));
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    // The value changed from outside (undo, clamping): show it.
    setShown(value);
    setText(String(value));
  }
  return (
    <span className="number-box">
      <input
        type="number"
        inputMode={Number.isInteger(step) ? "numeric" : "decimal"}
        step={step}
        min={min}
        max={max}
        aria-label={label}
        value={text}
        onChange={(e) => {
          const next = e.target.value;
          setText(next);
          if (next.trim() !== "" && Number.isFinite(Number(next))) onChange(Number(next));
        }}
        onBlur={() => setText(String(value))}
      />
      {unit && <span className="unit">{unit}</span>}
    </span>
  );
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

export function Select<T extends string | number>(props: { value: T; options: [T, string][]; onChange: (value: T) => void; label: string; disabled?: boolean }) {
  const { value, options, onChange, label, disabled } = props;
  return (
    <select
      aria-label={label}
      value={String(value)}
      disabled={disabled}
      onChange={(e) => {
        const picked = options.find(([option]) => String(option) === e.target.value);
        if (picked) onChange(picked[0]);
      }}
    >
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
  return (
    <span className="media-picker">
      <span className="media-name" title={file ?? undefined}>
        {file ? mediaFileName(file) : t("noFile")}
      </span>
      <button type="button" className="mini" aria-label={`${label}: ${t("chooseFile")}`} title={t("chooseFile")} onClick={onChoose}>
        …
      </button>
      {file && (
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
    // Focus the control marked for it, or else the first field, so typing starts at once.
    const body = box.current?.querySelector(".dialog-body");
    (body?.querySelector<HTMLElement>("[data-autofocus]") ?? body?.querySelector<HTMLElement>("textarea, input, select, button"))?.focus();
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

/** A box over the window at a point (kept inside the window); closes on any click outside or Escape. */
export function Popup(props: { x: number; y: number; onClose: () => void; className: string; role: string; label?: string; children: ReactNode }) {
  const { x, y, onClose, className, role, label, children } = props;
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const element = box.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    element.style.left = `${Math.max(0, Math.min(x, window.innerWidth - rect.width - 4))}px`;
    element.style.top = `${Math.max(0, Math.min(y, window.innerHeight - rect.height - 4))}px`;
    element.focus();
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
          if (e.key === "Escape") onClose();
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
