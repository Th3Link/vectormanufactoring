import { RotateCcw } from "lucide-react";
import { useState } from "react";
import type { KeyboardEvent } from "react";

import { EntryField } from "@/components/panel/EntryField";
import { Tooltip } from "@/components/ui/tooltip";
import { gridOf, useValueDrag } from "@/hooks/useValueDrag";
import type { StylePanelApi, ValueFieldName } from "@/hooks/useStylePanel";

const TOOLTIP_KEY = /Mac|iPhone|iPad/.test(navigator.platform) ? "Cmd" : "Ctrl";
const TOOLTIP = `Drag to change, click to type. Shift: coarse. ${TOOLTIP_KEY}: fine. ${TOOLTIP_KEY}+Backspace: reset.`;

interface ValueFieldProps {
  /** The visible label inside the field ("Width", "Opacity"). */
  label: string;
  /** The accessible name ("Stroke width"); contains the label. */
  name: string;
  field: ValueFieldName;
  /** The value as shown, without the unit; ignored when mixed. */
  text: string;
  /** The share of the field's width the bar fills, 0 to 1. */
  bar: number;
  mixed: boolean;
  /** The value is mixed or not the default: the reset icon shows. */
  resettable: boolean;
  /** The fixed unit ("mm", "%"). */
  unit: string;
  /** The number the value is, for `aria-valuenow`. */
  valueNow: number;
  /** The unit spelled out for `aria-valuetext` ("millimetres", "percent"). */
  unitWords: string;
  /** The largest value that may be typed, for `aria-valuemax`. */
  typedMax: number;
  /** The reset target as text ("0.25 mm", "100 %"). */
  defaultText: string;
  /** The message of a refused typed value. */
  messages: Readonly<Record<string, string>>;
  panel: StylePanelApi;
  onReturnFocus: () => void;
}

/** Characters that start typing in a focused value field (criterion 43). */
const STARTS_TYPING = /^[0-9.,-]$/;

/**
 * A value field (`specs/0017-style-panel-rework` criteria 34 to 45, 61,
 * `docs/design-system.md`, "Value field"): the label at the left, the value and
 * its unit at the right, a bar filled to the value, and a reserved 24 px slot at
 * the right end for the reset icon. It is a `div` with `role="spinbutton"`,
 * not an input, until the maker clicks it to type; then it renders the shared
 * typed-text field. Dragging, the scales and the rounding are Rust's: this
 * sends the position along the field and the modifier grid.
 */
export function ValueField({
  label,
  name,
  field,
  text,
  bar,
  mixed,
  resettable,
  unit,
  valueNow,
  unitWords,
  typedMax,
  defaultText,
  messages,
  panel,
  onReturnFocus,
}: ValueFieldProps) {
  const [typing, setTyping] = useState<{ initial?: string } | null>(null);

  const { modifier, handlers } = useValueDrag({
    bar: mixed ? 0 : bar,
    mixed,
    onTick: (p, grid) => panel.previewValue(field, p, grid),
    onClick: () => setTyping({}),
    onDragEnd: onReturnFocus,
  });

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const grid = gridOf(event);
    const step = (steps: number) => {
      event.preventDefault();
      panel.stepValue(field, steps, grid);
    };
    if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      step(1);
    } else if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      step(-1);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      panel.previewValue(field, event.key === "Home" ? 0 : 1, "normal");
    } else if ((event.ctrlKey || event.metaKey) && event.key === "Backspace") {
      event.preventDefault();
      panel.resetValue(field);
    } else if (event.key === "Backspace" || event.key === "Delete") {
      // Never reset, never reach the canvas.
      event.preventDefault();
      event.stopPropagation();
    } else if (event.key === "Enter" || event.key === "F2") {
      event.preventDefault();
      setTyping({});
    } else if (!event.ctrlKey && !event.metaKey && STARTS_TYPING.test(event.key)) {
      event.preventDefault();
      setTyping({ initial: event.key });
    }
  };

  const unitText = mixed ? "" : unit;
  return (
    <div className="relative h-7 w-[244px]">
      {typing ? (
        <>
          <EntryField
            label={name}
            shown={mixed ? "" : text}
            mixed={mixed}
            suffix={unit}
            suffixRight={30}
            gutter={30 + (unit.length > 1 ? 24 : 12) + 4}
            width={244}
            autoFocus
            initialText={typing.initial}
            onSubmit={(typed) => panel.setText(field, typed)}
            messages={messages}
            onReturnFocus={onReturnFocus}
            onClose={() => setTyping(null)}
          />
          <span
            aria-hidden
            className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-sm text-[var(--toolbar-icon)]"
          >
            {label}
          </span>
        </>
      ) : (
        <Tooltip side="left" content={TOOLTIP}>
          <div
            role="spinbutton"
            tabIndex={0}
            aria-label={name}
            aria-valuemin={0}
            aria-valuemax={typedMax}
            aria-valuenow={mixed ? undefined : valueNow}
            aria-valuetext={mixed ? "Mixed" : `${text} ${unitWords}`}
            aria-keyshortcuts="Control+Backspace"
            onKeyDown={onKeyDown}
            {...handlers}
            className="relative h-7 w-[244px] cursor-ew-resize overflow-hidden rounded-[5px] border border-[color-mix(in_srgb,var(--toolbar-icon)_60%,transparent)] bg-white outline-none hover:border-[var(--toolbar-icon)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)]"
            style={{ touchAction: "pan-y" }}
          >
            {!mixed && bar > 0 && (
              <span
                aria-hidden
                className="absolute inset-y-0 left-0 border-r-2 border-[var(--value-edge)] bg-[var(--value-fill)]"
                style={{ width: `${Math.min(1, bar) * 100}%` }}
              />
            )}
            <span
              aria-hidden
              className="absolute top-1/2 left-2 -translate-y-1/2 text-sm text-[var(--toolbar-icon)]"
            >
              {label}
            </span>
            {modifier !== "normal" && (
              <span
                aria-hidden
                className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 text-xs text-[var(--panel-muted-fg)]"
              >
                {modifier}
              </span>
            )}
            <span
              aria-hidden
              className="absolute top-1/2 right-8 -translate-y-1/2 text-sm tabular-nums"
            >
              {mixed ? (
                <span className="text-[var(--field-placeholder)]">Mixed</span>
              ) : (
                <>
                  <span className="text-[var(--toolbar-icon)]">{text}</span>
                  {unitText && (
                    <span className="ml-1 text-[var(--panel-muted-fg)]">{unitText}</span>
                  )}
                </>
              )}
            </span>
          </div>
        </Tooltip>
      )}
      {resettable && !typing && (
        <Tooltip side="left" content={`Reset to ${defaultText} (${TOOLTIP_KEY}+Backspace)`}>
          <button
            type="button"
            tabIndex={-1}
            aria-label={`Reset ${name.toLowerCase()} to ${defaultText}`}
            onClick={(event) => {
              panel.resetValue(field);
              // `detail` is 0 for keyboard activation, 1 or more for a click.
              if (event.detail > 0) {
                onReturnFocus();
              }
            }}
            className="absolute top-0.5 right-1 flex size-6 items-center justify-center rounded-[4px] text-[var(--toolbar-icon)] opacity-80 outline-none hover:bg-[var(--editor-accent-hover)]"
          >
            <RotateCcw size={12} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
      )}
    </div>
  );
}
