import { Tooltip } from "radix-ui";
import { forwardRef } from "react";

import { BooleanGlyph } from "@/components/BooleanGlyphs";
import {
  type BooleanAvailabilityState,
  type BooleanOp,
  operationName,
  operationRule,
  tooltipNote,
} from "@/lib/booleanText";

interface BooleanButtonProps {
  op: BooleanOp;
  availability: BooleanAvailabilityState;
  /** Whether the Select tool is active: the tooltip says which tool to take otherwise. */
  selectTool: boolean;
  /** The roving tab stop: 0 for the button the Tab key lands on, else -1. */
  tabIndex: number;
  /** The operation is running and this is the pressed button. */
  pressed: boolean;
  onFocus: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
  /** An activation of the enabled button; `byMouse` tells a click from a key. */
  onActivate: (byMouse: boolean) => void;
}

/**
 * One button of the Boolean section (`docs/design-system.md`, rows "Boolean tool section" and
 * "Boolean tooltip"): icon only, a command and not a tool, dimmed with `aria-disabled` while the
 * session says there is nothing to do, with a three-line tooltip (also on keyboard focus).
 */
export const BooleanButton = forwardRef<HTMLButtonElement, BooleanButtonProps>(
  function BooleanButton(
    { op, availability, selectTool, tabIndex, pressed, onFocus, onKeyDown, onActivate },
    ref,
  ) {
    const dimmed = availability.needsTwo;
    return (
      <Tooltip.Root delayDuration={400}>
        <Tooltip.Trigger asChild>
          <button
            ref={ref}
            type="button"
            aria-label={operationName(op)}
            aria-disabled={dimmed}
            tabIndex={tabIndex}
            onFocus={onFocus}
            onKeyDown={onKeyDown}
            onPointerDown={(event) => {
              // Radix closes a tooltip on a press; a press on a dimmed button leaves it
              // open, the tooltip says why.
              if (dimmed) {
                event.preventDefault();
              }
            }}
            onClick={(event) => {
              if (dimmed) {
                event.preventDefault();
                return;
              }
              // `detail` is 0 for keyboard activation, 1 or more for a mouse click.
              onActivate(event.detail > 0);
            }}
            data-pressed={pressed ? "true" : undefined}
            className={`flex size-10 items-center justify-center rounded-md outline-none focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)] data-[pressed=true]:bg-[var(--editor-accent-hover)] ${
              dimmed
                ? "opacity-40"
                : "hover:bg-[var(--editor-accent-hover)] active:bg-[var(--editor-accent-hover)]"
            }`}
            style={{ color: "var(--toolbar-icon)" }}
          >
            <BooleanGlyph op={op} />
          </button>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="right"
            sideOffset={6}
            className="z-50 max-w-[260px] rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground ring-1 ring-foreground/10"
          >
            <div className="font-semibold whitespace-nowrap">{operationName(op)}</div>
            <div className="whitespace-nowrap">{operationRule(op)}</div>
            <div style={{ color: "var(--panel-muted-fg)" }}>
              {tooltipNote(availability, selectTool)}
            </div>
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    );
  },
);
