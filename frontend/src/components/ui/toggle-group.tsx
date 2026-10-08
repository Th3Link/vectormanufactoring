import { RadioGroup } from "radix-ui";
import type { ReactNode } from "react";

import { Tooltip } from "@/components/ui/tooltip";

export interface ToggleOption<T extends string> {
  value: T;
  /** The accessible name ("Miter join"). */
  label: string;
  /** The tooltip text. */
  tooltip: string;
  icon: ReactNode;
}

interface ToggleGroupProps<T extends string> {
  /** The accessible name of the group ("Stroke join"). */
  label: string;
  options: readonly ToggleOption<T>[];
  /** The pressed item; `null` shows none (mixed, or a value without a button). */
  value: T | null;
  onChange: (value: T) => void;
  disabled?: boolean;
  /** Item width in px: 40 for Join and Cap, 44 for Paint and Fill type. */
  itemWidth: number;
  /** Item height in px: 28, or 32 for Fill type. */
  itemHeight?: number;
  /** After a mouse click, so the letter keys keep working. */
  onReturnFocus: () => void;
  /** Marks the group's tab stop as the panel's first focus target. */
  firstFocus?: boolean;
}

const BORDER = "border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]";

/**
 * A strip of radio buttons (`docs/design-system.md`, "`ToggleGroup` item") on
 * Radix's `RadioGroup`: one Tab stop per group, the arrow keys move focus and
 * select, Home and End jump and select. The tooltip trigger shares the item's
 * element and replaces its `data-state` ("closed", "delayed-open"), so the
 * pressed look is keyed on `aria-checked`, which nothing else touches. One 1 px
 * bordered strip; the pressed item on
 * `--toolbar-icon-active-bg`, or, when the group is disabled, on
 * `--toolbar-icon` at 30%.
 */
export function ToggleGroup<T extends string>({
  label,
  options,
  value,
  onChange,
  disabled = false,
  itemWidth,
  itemHeight = 28,
  onReturnFocus,
  firstFocus = false,
}: ToggleGroupProps<T>) {
  return (
    <RadioGroup.Root
      // With nothing pressed the first item is the Tab stop (Radix would keep the
      // one it remembers): a new group starts over.
      key={value === null ? "none" : "set"}
      aria-label={label}
      orientation="horizontal"
      disabled={disabled}
      value={value ?? ""}
      onValueChange={(next) => onChange(next as T)}
      onKeyDownCapture={(event) => {
        // Home and End select the first and last item, as the arrows select
        // the next: Radix moves focus on them but does not select.
        if (event.key !== "Home" && event.key !== "End") {
          return;
        }
        const items = Array.from(
          event.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]:not(:disabled)'),
        );
        const target = event.key === "Home" ? items[0] : items[items.length - 1];
        if (target) {
          event.preventDefault();
          event.stopPropagation();
          target.focus();
          onChange(target.dataset.value as T);
        }
      }}
      className={`inline-flex w-fit shrink-0 self-start overflow-hidden rounded-[5px] border ${BORDER}`}
    >
      {options.map((option, index) => (
        <Tooltip key={option.value} side="left" content={option.tooltip}>
          <RadioGroup.Item
            value={option.value}
            data-value={option.value}
            aria-label={option.label}
            // The one Tab stop: the pressed item, or the first when none is.
            data-first-focus={
              firstFocus && (option.value === value || (value === null && index === 0))
                ? ""
                : undefined
            }
            onClick={(event) => {
              // `detail` is 0 for keyboard activation, 1 or more for a click.
              if (event.detail > 0) {
                onReturnFocus();
              }
            }}
            className={`flex items-center justify-center bg-transparent text-[var(--toolbar-icon)] outline-none enabled:hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)] aria-checked:enabled:!bg-[var(--toolbar-icon-active-bg)] aria-checked:enabled:!text-[var(--toolbar-icon-active-fg)] aria-checked:disabled:bg-[color-mix(in_srgb,var(--toolbar-icon)_30%,transparent)] ${
              index > 0 ? `border-l ${BORDER}` : ""
            }`}
            style={{ width: itemWidth, height: itemHeight }}
          >
            {option.icon}
          </RadioGroup.Item>
        </Tooltip>
      ))}
    </RadioGroup.Root>
  );
}
