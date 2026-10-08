import { useRef, useState } from "react";
import { HsvaColorPicker } from "react-colorful";
import type { HsvaColor } from "react-colorful";

import { NumberField } from "@/components/NumberField";
import { Swatch } from "@/components/Swatch";
import { StyleRow } from "@/components/StyleRow";
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip } from "@/components/ui/tooltip";
import type { StyleFieldName, StylePanelApi } from "@/hooks/useStylePanel";
import { hsvToRgb, percentText, rgbToHsv, toHex } from "@/lib/styleColor";

const HEX_MESSAGES = {
  hex: "Enter 3 or 6 hex digits",
  hex8: "Use 6 digits; set opacity separately",
} as const;
const PERCENT_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

/** The three sliders react-colorful renders, in DOM order, with the names the
 * design system gives them. It labels them "Color", "Hue" and "Alpha". */
const SLIDER_NAMES = ["Saturation and value", "Hue", "Opacity"] as const;

/** Shift with an arrow key is a 10 % step; react-colorful steps 1 % only. */
function shiftStep(
  hsva: HsvaColor,
  slider: number,
  key: string,
): HsvaColor | null {
  const sign = key === "ArrowRight" || key === "ArrowUp" ? 1 : -1;
  const clamp = (n: number, max: number) => Math.min(max, Math.max(0, n));
  const horizontal = key === "ArrowLeft" || key === "ArrowRight";
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(key)) {
    return null;
  }
  if (slider === 0) {
    return horizontal
      ? { ...hsva, s: clamp(hsva.s + sign * 10, 100) }
      : { ...hsva, v: clamp(hsva.v + sign * 10, 100) };
  }
  if (slider === 1 && horizontal) {
    return { ...hsva, h: clamp(hsva.h + sign * 36, 360) };
  }
  if (slider === 2 && horizontal) {
    return { ...hsva, a: clamp(hsva.a + sign * 0.1, 1) };
  }
  return null;
}

interface PickerProps {
  /** "Stroke" or "Fill": names the controls. */
  name: "Stroke" | "Fill";
  rgb: number;
  rgbMixed: boolean;
  opacity: number;
  opacityMixed: boolean;
  /** The paint is off (a stroke with Paint = None): the swatch is slashed. */
  off?: boolean;
  disabled: boolean;
  colorField: StyleFieldName;
  opacityField: StyleFieldName;
  panel: Pick<StylePanelApi, "setText" | "previewColor" | "previewOpacity">;
  /** Changes when the edited objects or the tool change: closes the popover. */
  closeKey: string;
  onReturnFocus: () => void;
}

/**
 * Colour and opacity of one paint (`docs/design-system.md`, "Swatch", "Colour
 * popover"): a swatch that opens the popover, the hex field and the opacity
 * field. Hex and opacity are inline so the common edits need no popover, and
 * they update live while the popover is dragged. The popover keeps its own HSV
 * state and re-derives it from the colour only when that changes from outside
 * it, so the hue does not jump when the pointer passes through a grey.
 */
export function ColorAlphaPicker({
  name,
  rgb,
  rgbMixed,
  opacity,
  opacityMixed,
  off = false,
  disabled,
  colorField,
  opacityField,
  panel,
  closeKey,
  onReturnFocus,
}: PickerProps) {
  const [open, setOpen] = useState(false);
  const openedByPointer = useRef(false);
  const shownRgb = rgbMixed ? 0 : rgb;
  const shownPercent = opacityMixed ? 100 : Math.round(opacity);

  const fromProps = (): HsvaColor => ({ ...rgbToHsv(shownRgb), a: shownPercent / 100 });
  const [hsva, setHsva] = useState<HsvaColor>(fromProps);
  const [seen, setSeen] = useState({ rgb: shownRgb, percent: shownPercent });
  if (seen.rgb !== shownRgb || seen.percent !== shownPercent) {
    setSeen({ rgb: shownRgb, percent: shownPercent });
    const emitted = hsvToRgb(hsva.h, hsva.s, hsva.v);
    if (emitted !== shownRgb || Math.round(hsva.a * 100) !== shownPercent) {
      setHsva(fromProps());
    }
  }

  // A selection or tool change closes the popover (criterion 36).
  const [openFor, setOpenFor] = useState(closeKey);
  if (openFor !== closeKey) {
    setOpenFor(closeKey);
    setOpen(false);
  }

  const change = (next: HsvaColor) => {
    if (next.h !== hsva.h || next.s !== hsva.s || next.v !== hsva.v) {
      panel.previewColor(colorField, hsvToRgb(next.h, next.s, next.v));
    }
    if (next.a !== hsva.a) {
      panel.previewOpacity(opacityField, Math.round(next.a * 100));
    }
    setHsva(next);
  };

  const description = rgbMixed || opacityMixed ? "mixed" : `${toHex(rgb)}, ${shownPercent}% opacity`;
  const tooltip = off
    ? `${name} is off. Choose a color to turn it on.`
    : `${name} color`;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      {/* The popover opens beside the whole row, so it clears the panel and
          never covers the hex and opacity fields it updates. */}
      <PopoverAnchor asChild>
        <StyleRow label="Color">
        <Tooltip content={tooltip}>
          <PopoverTrigger asChild>
            <Swatch
              rgb={shownRgb}
              opacity={shownPercent}
              mixed={rgbMixed}
              off={off}
              disabled={disabled}
              aria-label={`${name} color`}
              aria-description={description}
              onClick={(event) => {
                // `detail` is 0 for keyboard activation, 1 or more for a click.
                openedByPointer.current = event.detail > 0;
              }}
            />
          </PopoverTrigger>
        </Tooltip>
        <PopoverContent
          aria-label={`${name} color picker`}
          onCloseAutoFocus={(event) => {
            // Opened with the pointer: focus goes to the canvas, so the tool
            // letters work again; opened from the keyboard it returns to the
            // swatch.
            if (openedByPointer.current) {
              event.preventDefault();
              onReturnFocus();
            }
          }}
        >
          <div
            className="colour-picker"
            ref={(element) => {
              element?.querySelectorAll('[role="slider"]').forEach((slider, index) => {
                const label = SLIDER_NAMES[index];
                if (label) {
                  slider.setAttribute("aria-label", label);
                }
              });
            }}
            onKeyDownCapture={(event) => {
              if (!event.shiftKey) {
                return;
              }
              const sliders = Array.from(
                event.currentTarget.querySelectorAll('[role="slider"]'),
              );
              const index = sliders.indexOf(event.target as Element);
              const next = index >= 0 ? shiftStep(hsva, index, event.key) : null;
              if (next) {
                event.preventDefault();
                event.stopPropagation();
                change(next);
              }
            }}
          >
            <HsvaColorPicker color={hsva} onChange={change} />
          </div>
        </PopoverContent>
      <NumberField
        label={`${name} color hex`}
        shown={toHex(rgb)}
        mixed={rgbMixed}
        width={84}
        align="left"
        disabled={disabled}
        onSubmit={(text) => panel.setText(colorField, text)}
        messages={HEX_MESSAGES}
        onReturnFocus={onReturnFocus}
      />
      <NumberField
        label={`${name} opacity percent`}
        shown={percentText(opacity)}
        mixed={opacityMixed}
        suffix="%"
        width={56}
        disabled={disabled}
        onSubmit={(text) => panel.setText(opacityField, text)}
        messages={PERCENT_MESSAGES}
        onReturnFocus={onReturnFocus}
      />
        </StyleRow>
      </PopoverAnchor>
    </Popover>
  );
}
