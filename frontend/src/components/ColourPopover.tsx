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

/** An arrow key on one of the picker's sliders: 1 % of its range, Shift 10 %
 * (criterion 36; react-colorful's own step is 5 %). `slider` is the index in
 * [`SLIDER_NAMES`]; `null` for a key that is not a step of that slider. */
function arrowStep(
  hsva: HsvaColor,
  slider: number,
  key: string,
  shift: boolean,
): HsvaColor | null {
  const unit = shift ? 10 : 1;
  const clamp = (n: number, max: number) => Math.min(max, Math.max(0, n));
  const sign = key === "ArrowRight" || key === "ArrowUp" ? 1 : -1;
  const horizontal = key === "ArrowLeft" || key === "ArrowRight";
  if (!horizontal && key !== "ArrowUp" && key !== "ArrowDown") {
    return null;
  }
  if (slider === 0) {
    return horizontal
      ? { ...hsva, s: clamp(hsva.s + sign * unit, 100) }
      : { ...hsva, v: clamp(hsva.v + sign * unit, 100) };
  }
  if (slider === 1 && horizontal) {
    return { ...hsva, h: clamp(hsva.h + sign * unit * 3.6, 360) };
  }
  if (slider === 2 && horizontal) {
    return { ...hsva, a: clamp(hsva.a + (sign * unit) / 100, 1) };
  }
  return null;
}

interface ColourPopoverProps {
  /** The committed colour, packed `0xRRGGBB` (0 when mixed). */
  rgb: number;
  /** The committed opacity percent, rounded (100 when mixed). */
  percent: number;
  /** A drag is running: the committed values lag the picker by a frame, so
   * they must not reset it (that would make the hue jump through a grey). */
  previewing: boolean;
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
  previewing,
  onPreviewColor,
  onPreviewOpacity,
}: ColourPopoverProps) {
  const fromProps = (): HsvaColor => ({ ...rgbToHsv(rgb), a: percent / 100 });
  const [hsva, setHsva] = useState<HsvaColor>(fromProps);
  const [seen, setSeen] = useState({ rgb, percent });
  if (!previewing && (seen.rgb !== rgb || seen.percent !== percent)) {
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
        const sliders = Array.from(event.currentTarget.querySelectorAll('[role="slider"]'));
        const index = sliders.indexOf(event.target as Element);
        const next = index >= 0 ? arrowStep(hsva, index, event.key, event.shiftKey) : null;
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
