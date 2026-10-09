import { EntryField } from "@/components/panel/EntryField";
import { Swatch } from "@/components/panel/Swatch";
import type { StylePanelApi } from "@/hooks/useStylePanel";

const HEX_MESSAGES = { hex: "Enter 3, 4, 6 or 8 hex digits" } as const;

interface ColourBlockProps {
  /** "Stroke" or "Fill": prefixes the accessible names. */
  name: string;
  /** `"stroke-color"` or `"fill-color"`: the field `set_style_text` takes. */
  field: "stroke-color" | "fill-color";
  rgb: number;
  hex: string;
  /** The edited objects differ in colour or alpha. */
  hexMixed: boolean;
  /** Opacity percent, for the swatch. */
  opacity: number;
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/**
 * The Color row of one paint (`specs/0017-style-panel-rework` criteria 11 to
 * 16): the swatch, which only shows the colour at its alpha, and the hex field
 * as `#RRGGBBAA`. A typed 3, 4, 6 or 8 digit form is read by Rust and shown
 * back in the canonical form.
 */
export function ColourBlock({
  name,
  field,
  rgb,
  hex,
  hexMixed,
  opacity,
  panel,
  onReturnFocus,
}: ColourBlockProps) {
  return (
    <div className="flex h-7 items-center gap-1">
      <Swatch rgb={rgb} opacity={opacity} mixed={hexMixed} />
      <EntryField
        label={`${name} color hex (RRGGBBAA)`}
        shown={hex}
        mixed={hexMixed}
        width={212}
        align="left"
        onSubmit={(text) => panel.setText(field, text)}
        messages={HEX_MESSAGES}
        onReturnFocus={onReturnFocus}
      />
    </div>
  );
}
