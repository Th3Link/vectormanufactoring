import type { KeyboardEvent, ReactNode } from "react";
import { useRef } from "react";

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
  /** Marks the group's first item as the panel's first focus target. */
  firstFocus?: boolean;
}

const ITEM_RING =
  "focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--editor-accent)]";

/**
 * A strip of radio buttons (`docs/design-system.md`, "`ToggleGroup` item"):
 * one 1 px bordered strip, one Tab stop per group, arrows move focus and
 * select, the pressed item on `--toolbar-icon-active-bg` (or, when the group
 * is disabled, on `--toolbar-icon` at 30%). Built by hand, not on Radix's
 * `ToggleGroup`, because that one moves focus with the arrows but selects only
 * on activation, and a radio group selects as it moves.
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
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const current = options.findIndex((o) => o.value === value);
  // The one Tab stop: the pressed item, or the first when none is pressed.
  const stop = current >= 0 ? current : 0;

  const move = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const step =
      event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowLeft" || event.key === "ArrowUp"
          ? -1
          : 0;
    if (step === 0) {
      return;
    }
    event.preventDefault();
    const next = (index + step + options.length) % options.length;
    items.current[next]?.focus();
    onChange(options[next].value);
  };

  return (
    <div
      role="radiogroup"
      aria-label={label}
      aria-disabled={disabled || undefined}
      className="inline-flex w-fit shrink-0 self-start overflow-hidden rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]"
    >
      {options.map((option, index) => {
        const pressed = index === current;
        return (
          <Tooltip key={option.value} content={option.tooltip}>
            <button
              ref={(element) => {
                items.current[index] = element;
              }}
              type="button"
              role="radio"
              aria-checked={pressed}
              aria-label={option.label}
              disabled={disabled}
              tabIndex={index === stop ? 0 : -1}
              data-first-focus={firstFocus && index === stop ? "" : undefined}
              onClick={(event) => {
                onChange(option.value);
                // `detail` is 0 for keyboard activation, 1 or more for a click.
                if (event.detail > 0) {
                  onReturnFocus();
                }
              }}
              onKeyDown={(event) => move(event, index)}
              className={`flex items-center justify-center bg-transparent text-[var(--toolbar-icon)] outline-none enabled:hover:bg-[var(--editor-accent-hover)] ${ITEM_RING} ${
                index > 0
                  ? "border-l border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)]"
                  : ""
              } ${
                pressed && !disabled
                  ? "!bg-[var(--toolbar-icon-active-bg)] !text-[var(--toolbar-icon-active-fg)]"
                  : ""
              } ${
                pressed && disabled
                  ? "!bg-[color-mix(in_srgb,var(--toolbar-icon)_30%,transparent)]"
                  : ""
              }`}
              style={{ width: itemWidth, height: itemHeight }}
            >
              {option.icon}
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
}
