import { CircleAlert } from "lucide-react";
import { Tooltip } from "radix-ui";
import { useRef, useState } from "react";

import { BooleanGlyph } from "@/components/BooleanGlyphs";
import type { BooleanCommands as BooleanCommandsState } from "@/hooks/useBooleanCommands";
import {
  BOOLEAN_OPS,
  type BooleanOp,
  operationName,
  operationRule,
  tooltipNote,
} from "@/lib/booleanText";

interface BooleanCommandsProps {
  commands: BooleanCommandsState;
  /** Whether the Select tool is active: with another tool the buttons are
   * dimmed and the tooltip says which tool to take. */
  selectTool: boolean;
  /** Called after a mouse press, so the tool letters keep working; a key
   * press leaves the focus on the button (`0016` criterion 22a). */
  onReturnFocus: () => void;
}

/**
 * The Boolean section of the tool rail (`docs/design-system.md`, rows "Boolean
 * tool section", "Boolean tooltip" and "Action notice"): five commands, not
 * tools, behind a divider. One Tab stop with roving focus; the notice of the
 * last operation sits beside the rail. It shows what the session says is
 * available and decides nothing.
 */
export function BooleanCommands({ commands, selectTool, onReturnFocus }: BooleanCommandsProps) {
  const { availability, busy, notice, apply } = commands;
  const dimmed = availability.needsTwo;
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const [stop, setStop] = useState(0);
  const [pressed, setPressed] = useState<BooleanOp | null>(null);

  const moveFocus = (to: number) => {
    const next = Math.min(Math.max(to, 0), BOOLEAN_OPS.length - 1);
    buttons.current[next]?.focus();
  };

  return (
    <>
      <div
        aria-hidden="true"
        className="my-1 h-px w-6"
        style={{ background: "color-mix(in srgb, var(--toolbar-icon) 25%, transparent)" }}
      />
      <div className="relative flex w-full flex-col items-center">
        <div
          role="toolbar"
          aria-label="Boolean operations"
          aria-orientation="vertical"
          aria-busy={busy}
          className="flex flex-col items-center gap-1"
        >
          {BOOLEAN_OPS.map((op, index) => (
            <Tooltip.Root key={op} delayDuration={400}>
              <Tooltip.Trigger asChild>
                <button
                  ref={(element) => {
                    buttons.current[index] = element;
                  }}
                  type="button"
                  aria-label={operationName(op)}
                  aria-disabled={dimmed}
                  tabIndex={index === stop ? 0 : -1}
                  onFocus={() => setStop(index)}
                  onPointerDown={(event) => {
                    // Radix closes a tooltip on a press; a press on a dimmed
                    // button leaves it open, the tooltip says why.
                    if (dimmed) {
                      event.preventDefault();
                    }
                  }}
                  onClick={(event) => {
                    if (dimmed || busy) {
                      event.preventDefault();
                      return;
                    }
                    setPressed(op);
                    apply(op);
                    // `detail` is 0 for keyboard activation, 1 or more for a mouse click.
                    if (event.detail > 0) {
                      onReturnFocus();
                    }
                  }}
                  onKeyDown={(event) => {
                    const keys: Record<string, number> = {
                      ArrowDown: index + 1,
                      ArrowUp: index - 1,
                      Home: 0,
                      End: BOOLEAN_OPS.length - 1,
                    };
                    if (event.key in keys) {
                      event.preventDefault();
                      moveFocus(keys[event.key]);
                    }
                  }}
                  data-pressed={busy && pressed === op ? "true" : undefined}
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
          ))}
        </div>
        {/* Both live regions are in the tree from the start, so a notice is
         * announced as a change of text. */}
        <div
          className="pointer-events-none absolute top-0 left-[60px] z-30 flex w-max max-w-[min(360px,calc(100vw-96px))] flex-col gap-1 text-xs"
          data-boolean-notice
        >
          <div role="status">
            {notice?.kind === "success" ? (
              <div
                className="rounded-lg p-2"
                style={{
                  background: "var(--toolbar-bg)",
                  color: "var(--toolbar-icon)",
                  boxShadow: "var(--panel-elevation-shadow)",
                }}
              >
                {notice.text}
              </div>
            ) : null}
          </div>
          <div role="alert">
            {notice?.kind === "refusal" ? (
              <div
                className="flex items-start gap-1.5 rounded-lg bg-popover p-2"
                style={{
                  color: "var(--field-invalid)",
                  boxShadow: "0 0 0 1px var(--field-invalid), var(--panel-elevation-shadow)",
                }}
              >
                <CircleAlert size={12} aria-hidden="true" className="mt-0.5 shrink-0" />
                <span>{notice.text}</span>
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </>
  );
}
