import { useState } from "react";
import { HsvaColorPicker } from "react-colorful";
import type { HsvaColor } from "react-colorful";

import { hsvToRgb, rgbToHsv } from "@/lib/styleColor";

/** The three sliders react-colorful renders, in DOM order, with the names the
 * design system gives them. It labels them "Color", "Hue" and "Alpha". */
const SLIDER_NAMES = ["Saturation and value", "Hue", "Opacity"] as const;

/** Names the picker's sliders after they mount. */
function renameSliders(element: HTMLDivElement | null) {
  element?.querySelectorAll('[role="slider"]').forEach((slider, index) => {
    const label = SLIDER_NAMES[index];
    if (label) {
      slider.setAttribute("aria-label", label);
    }
  });
}

/** Shift with an arrow key is a 10 % step; react-colorful steps 1 % only.
 * `slider` is the index in [`SLIDER_NAMES`]; `null` for a key that is not a step. */
function shiftStep(hsva: HsvaColor, slider: number, key: string): HsvaColor | null {
  const clamp = (n: number, max: number) => Math.min(max, Math.max(0, n));
  const sign = key === "ArrowRight" || key === "ArrowUp" ? 1 : -1;
  const horizontal = key === "ArrowLeft" || key === "ArrowRight";
  if (!horizontal && key !== "ArrowUp" && key !== "ArrowDown") {
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

interface ColourPopoverProps {
  /** The committed colour, packed `0xRRGGBB` (0 when mixed). */
  rgb: number;
  /** The committed opacity percent, rounded (100 when mixed). */
  percent: number;
  onPreviewColor: (rgb: number) => void;
  onPreviewOpacity: (percent: number) => void;
}

/**
 * The body of the colour popover (`docs/design-system.md`, "Colour popover"):
 * saturation/value area, hue and opacity sliders. It keeps its own HSV state
 * and re-derives it from the committed colour only when that changes from
 * outside it, so the hue does not jump when the pointer passes through a grey.
 */
export function ColourPopover({
  rgb,
  percent,
  onPreviewColor,
  onPreviewOpacity,
}: ColourPopoverProps) {
  const fromProps = (): HsvaColor => ({ ...rgbToHsv(rgb), a: percent / 100 });
  const [hsva, setHsva] = useState<HsvaColor>(fromProps);
  const [seen, setSeen] = useState({ rgb, percent });
  if (seen.rgb !== rgb || seen.percent !== percent) {
    setSeen({ rgb, percent });
    const emitted = hsvToRgb(hsva.h, hsva.s, hsva.v);
    if (emitted !== rgb || Math.round(hsva.a * 100) !== percent) {
      setHsva(fromProps());
    }
  }

  const change = (next: HsvaColor) => {
    if (next.h !== hsva.h || next.s !== hsva.s || next.v !== hsva.v) {
      onPreviewColor(hsvToRgb(next.h, next.s, next.v));
    }
    if (next.a !== hsva.a) {
      onPreviewOpacity(Math.round(next.a * 100));
    }
    setHsva(next);
  };

  return (
    <div
      className="colour-picker"
      ref={renameSliders}
      onKeyDownCapture={(event) => {
        if (!event.shiftKey) {
          return;
        }
        const sliders = Array.from(event.currentTarget.querySelectorAll('[role="slider"]'));
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
  );
}
