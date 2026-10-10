// Property rows of each kind: a labelled control with a description, and
// its value for Reset, Copy and Paste and to compare several slides.

import type { ReactNode } from "react";
import type { StringKey, Translate } from "../i18n";
import type { Color, Gradient, GradientKind } from "../types";
import { ColorBox } from "./ColorPicker";
import { NumberBox, Select, Switch } from "./controls";
import { FontBox } from "./FontPicker";
import { addMyGradient, GRADIENT_KINDS } from "./gradients";
import { GradientBox } from "./GradientPicker";
import type { FontValue, PropertyRow } from "./property-rows";

export type RowOptions = Pick<PropertyRow, "wide" | "rare">;

/** Changes a gradient as it is by then; `field` names what is dragged or typed, so it is one undo step. */
export type GradientChange = (change: (current: Gradient | null) => Gradient | null, field?: string) => void;

const GRADIENT_KIND_LABEL: Record<GradientKind, StringKey> = {
  vertical: "gradientVertical",
  horizontal: "gradientHorizontal",
  diagonal: "gradientDiagonal",
  diagonalUp: "gradientDiagonalUp",
  radial: "gradientRadial",
};

export function rowBuilders(t: Translate) {
  const row = (id: string, label: StringKey, description: StringKey, control: ReactNode, options: Partial<PropertyRow> = {}): PropertyRow => ({
    id,
    label: t(label),
    description: t(description),
    control,
    ...options,
  });

  /** A number: its box, with a slider when `slider`. */
  const numberRow = (
    id: string,
    label: StringKey,
    description: StringKey,
    number: { value: number; initial?: number; min: number; max: number; step?: number; unit?: string; slider?: boolean; set: (value: number) => void },
    options: RowOptions = {},
  ) => {
    const { value, initial, min, max, step = 1, unit, slider, set } = number;
    const control = <NumberBox label={t(label)} value={value} min={min} max={max} step={step} unit={unit} slider={slider} onChange={set} />;
    return row(id, label, description, control, { ...options, value: { kind: "number", value, initial, min, max, step, set } });
  };

  const flagRow = (
    id: string,
    label: StringKey,
    description: StringKey,
    flag: { value: boolean; initial?: boolean; disabled?: boolean; set: (on: boolean) => void },
    options: RowOptions = {},
  ) =>
    row(id, label, description, <Switch label={t(label)} checked={flag.value} disabled={flag.disabled} onChange={flag.set} />, {
      ...options,
      value: { kind: "flag", value: flag.value, initial: flag.initial, set: flag.set },
    });

  const choiceRow = <T extends string | number>(
    id: string,
    label: StringKey,
    description: StringKey,
    choice: { value: T; initial?: T; options: [T, string][]; disabled?: boolean; set: (value: T) => void },
    options: RowOptions = {},
  ) => {
    const { value, initial, options: offered, disabled, set } = choice;
    const control = <Select label={t(label)} value={value} options={offered} disabled={disabled} onChange={set} />;
    const setOffered = (picked: string | number) => {
      const option = offered.find(([option]) => option === picked);
      if (option) set(option[0]);
    };
    return row(id, label, description, control, {
      ...options,
      value: { kind: "choice", value, initial, options: offered.map(([option]) => option), set: setOffered },
    });
  };

  /** A colour; `none` names the empty value (the theme's colour), and without it there is none to pick. */
  const colorRow = (
    id: string,
    label: StringKey,
    description: StringKey,
    color: { value: Color | null; initial?: Color | null; none?: StringKey; set: (color: Color | null) => void },
    options: RowOptions = {},
  ) => {
    const { value, initial, none, set } = color;
    const control = <ColorBox label={t(label)} value={value} none={none && t(none)} onChange={set} />;
    return row(id, label, description, control, { ...options, value: { kind: "color", value, initial, set } });
  };

  /**
   * A gradient, summed up by its preview; its From, To and Type are its
   * parts. Setting a colour without a gradient makes one of that colour.
   */
  const gradientRow = (id: string, label: StringKey, description: StringKey, gradient: { value: Gradient | null; change: GradientChange }, options: RowOptions = {}) => {
    const { value, change } = gradient;
    const set = (next: Gradient | null) => change(() => next, id);
    const end = (side: "from" | "to", sideLabel: StringKey, sideDescription: StringKey) =>
      colorRow(`${id}.${side}`, sideLabel, sideDescription, {
        value: value?.[side] ?? null,
        set: (color) =>
          change((current) => {
            if (color === null) return null;
            const next: Gradient = { from: current?.from ?? color, to: current?.to ?? color, kind: current?.kind ?? "vertical" };
            next[side] = color;
            return next;
          }, `${id}.${side}`),
      });
    return row(id, label, description, <GradientBox label={t(label)} value={value} onChange={set} />, {
      ...options,
      value: { kind: "gradient", value, initial: null, set },
      parts: [
        end("from", "gradientFrom", "descGradientFrom"),
        end("to", "gradientTo", "descGradientTo"),
        choiceRow(`${id}.kind`, "gradientKind", "descGradientKind", {
          value: value?.kind ?? "vertical",
          options: GRADIENT_KINDS.map((kind) => [kind, t(GRADIENT_KIND_LABEL[kind])]),
          disabled: !value,
          set: (kind) => change((current) => current && { ...current, kind }),
        }),
      ],
      menu: [{ label: t("addToMyGradients"), disabled: !value, run: () => value && addMyGradient(value) }],
    });
  };

  /** A text's font, size, bold, italic and underline; what is left out is the theme's. */
  const fontRow = (
    id: string,
    label: StringKey,
    description: StringKey,
    font: { value: FontValue; themeBold: boolean; change: (change: Partial<FontValue>) => void },
    options: RowOptions = {},
  ) => {
    const { value, themeBold, change } = font;
    // Every field, so what the pasted font leaves out goes back to the theme's.
    const set = ({ font, size, bold, italic, underline }: FontValue) => change({ font, size, bold, italic, underline });
    return row(id, label, description, <FontBox label={t(label)} value={value} themeBold={themeBold} onChange={change} />, {
      wide: true,
      ...options,
      value: { kind: "font", value, initial: {}, set },
    });
  };

  return { row, numberRow, flagRow, choiceRow, colorRow, gradientRow, fontRow };
}
