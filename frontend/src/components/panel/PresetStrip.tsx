import { RadioGroup } from "radix-ui";
import type { ReactNode } from "react";

import { Tooltip } from "@/components/ui/tooltip";

export interface StripCell {
  id: string;
  /** The visible content: the name, or a glyph and a name. */
  content: ReactNode;
  /** The accessible name ("Paper A4"). */
  label: string;
  /** The tooltip: lines separated by a newline; the second is muted. */
  tooltip: ReactNode;
}

interface PresetStripProps {
  /** The id of the group heading that names the strip. */
  labelledBy: string;
  cells: readonly StripCell[];
  /** The pressed cell, or none. */
  value: string | null;
  /** A press on a cell, also on the pressed one (Rust decides what it writes);
   * `keyboard` is true for an arrow key, which moves and presses. */
  onPress: (id: string, keyboard: boolean) => void;
  /** After a mouse press the keyboard goes back to the canvas. */
  onReturnFocus: () => void;
}

const BORDER = "color-mix(in srgb, var(--toolbar-icon) 60%, transparent)";

/**
 * A strip of choices for the Document section (`specs/0030-document-size-
 * presets` UX notes): the look of the panel's `ToggleGroup` (one 1 px border,
 * 28 px high, 1 px between cells) with cells as wide as their names need, at
 * least 32 px, wrapping to a further row when they do not fit. Radix's
 * `RadioGroup` gives one Tab stop per group and arrows that move and press; a
 * press on the pressed cell is forwarded too, because Radix reports no change
 * for it.
 */
export function PresetStrip({ labelledBy, cells, value, onPress, onReturnFocus }: PresetStripProps) {
  return (
    <RadioGroup.Root
      aria-labelledby={labelledBy}
      orientation="horizontal"
      value={value ?? ""}
      // The press itself is the click handler below (it also covers the pressed
      // cell and the arrow keys, which Radix turns into a click).
      onValueChange={() => undefined}
      className="flex w-[244px] flex-wrap gap-px overflow-hidden rounded-[5px] border"
      style={{ borderColor: BORDER, background: BORDER }}
    >
      {cells.map((cell) => (
        <Tooltip key={cell.id} side="left" content={cell.tooltip}>
          <RadioGroup.Item
            value={cell.id}
            aria-label={cell.label}
            onClick={(event) => {
              // `detail` is 0 for keyboard activation, 1 or more for a click.
              onPress(cell.id, event.detail === 0);
              if (event.detail > 0) {
                onReturnFocus();
              }
            }}
            className="flex h-7 min-w-8 flex-[1_1_auto] items-center justify-center gap-1.5 bg-[var(--panel-bg)] px-1.5 text-sm text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)] aria-checked:!bg-[var(--toolbar-icon-active-bg)] aria-checked:!text-[var(--toolbar-icon-active-fg)]"
          >
            {cell.content}
          </RadioGroup.Item>
        </Tooltip>
      ))}
    </RadioGroup.Root>
  );
}
