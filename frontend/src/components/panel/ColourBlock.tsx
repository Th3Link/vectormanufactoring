import { Pipette } from "lucide-react";

import { EntryField } from "@/components/panel/EntryField";
import { Swatch } from "@/components/panel/Swatch";
import { Tooltip } from "@/components/ui/tooltip";
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
 * 16, 22): the swatch, which only shows the colour at its alpha, the
 * eyedropper button and the hex field as `#RRGGBBAA`. A typed 3, 4, 6 or 8 digit form is read by Rust and shown
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
  const paint = field === "stroke-color" ? "stroke" : "fill";
  const picking = panel.view.pickTarget === paint;
  return (
    <div className="flex h-7 items-center gap-1">
      <Swatch rgb={rgb} opacity={opacity} mixed={hexMixed} />
      <Tooltip side="left" content="Pick a color from the drawing (Esc cancels)">
        <button
          type="button"
          data-eyedropper=""
          aria-label={`Pick ${name.toLowerCase()} color from the drawing`}
          aria-pressed={picking}
          onClick={(event) => {
            panel.beginPick(paint);
            // `detail` is 0 for keyboard activation, 1 or more for a click. A
            // click goes on to the canvas, where the pick happens.
            if (event.detail > 0) {
              onReturnFocus();
            }
          }}
          className="flex size-7 shrink-0 items-center justify-center rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] aria-pressed:border-transparent aria-pressed:!bg-[var(--toolbar-icon-active-bg)] aria-pressed:!text-[var(--toolbar-icon-active-fg)]"
        >
          <Pipette size={16} strokeWidth={1.5} aria-hidden />
        </button>
      </Tooltip>
      <EntryField
        label={`${name} color hex (RRGGBBAA)`}
        shown={hex}
        mixed={hexMixed}
        width={176}
        align="left"
        onSubmit={(text) => panel.setText(field, text)}
        messages={HEX_MESSAGES}
        onReturnFocus={onReturnFocus}
      />
    </div>
  );
}
