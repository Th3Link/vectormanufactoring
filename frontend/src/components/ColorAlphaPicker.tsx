import { useRef, useState } from "react";

import { ColourPopover } from "@/components/ColourPopover";
import { NumberField } from "@/components/NumberField";
import { StyleRow } from "@/components/StyleRow";
import { Swatch } from "@/components/Swatch";
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip } from "@/components/ui/tooltip";
import type { StyleFieldName, StylePanelApi } from "@/hooks/useStylePanel";
import { percentText, toHex } from "@/lib/styleColor";

const HEX_MESSAGES = {
  hex: "Enter 3 or 6 hex digits",
  hex8: "Use 6 digits; set opacity separately",
} as const;
const PERCENT_MESSAGES = { percent: "Enter a number from 0 to 100" } as const;

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
 * The "Color" row of one paint (`docs/design-system.md`, "Swatch"): a swatch
 * that opens the colour popover, the hex field and the opacity field. Hex and
 * opacity are inline so the common edits need no popover, and they update live
 * while the popover is dragged. The popover opens beside the whole row, so it
 * clears the panel and never covers the fields it updates.
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

  // A selection or tool change closes the popover (criterion 36).
  const [openFor, setOpenFor] = useState(closeKey);
  if (openFor !== closeKey) {
    setOpenFor(closeKey);
    setOpen(false);
  }

  const description =
    rgbMixed || opacityMixed ? "mixed" : `${toHex(rgb)}, ${shownPercent}% opacity`;
  const tooltip = off ? `${name} is off. Choose a color to turn it on.` : `${name} color`;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <StyleRow label="Color">
          <Tooltip side="left" content={tooltip}>
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
            <ColourPopover
              rgb={shownRgb}
              percent={shownPercent}
              onPreviewColor={(next) => panel.previewColor(colorField, next)}
              onPreviewOpacity={(next) => panel.previewOpacity(opacityField, next)}
            />
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
