import { useRef, useState } from "react";

import { BooleanButton } from "@/components/BooleanButton";
import { BooleanNotice } from "@/components/BooleanNotice";
import type { BooleanCommands as BooleanCommandsState } from "@/hooks/useBooleanCommands";
import { BOOLEAN_OPS, type BooleanOp } from "@/lib/booleanText";

interface BooleanCommandsProps {
  commands: BooleanCommandsState;
  /** Whether the Select tool is active: with another tool the buttons are dimmed and the
   * tooltip says which tool to take. */
  selectTool: boolean;
  /** Called after a mouse press, so the tool letters keep working; a key press leaves the
   * focus on the button (`0016` criterion 22a). */
  onReturnFocus: () => void;
}

/** The index a navigation key moves the focus to (no wrap), or `null` for any other key. */
function targetIndex(key: string, index: number): number | null {
  const last = BOOLEAN_OPS.length - 1;
  switch (key) {
    case "ArrowDown":
      return Math.min(index + 1, last);
    case "ArrowUp":
      return Math.max(index - 1, 0);
    case "Home":
      return 0;
    case "End":
      return last;
    default:
      return null;
  }
}

/**
 * The Boolean section of the tool rail (`docs/design-system.md`, row "Boolean tool section"):
 * five commands, not tools, behind a divider. One Tab stop with roving focus; the notice of the
 * last operation sits beside the rail. It shows what the session says is available and decides
 * nothing.
 */
export function BooleanCommands({ commands, selectTool, onReturnFocus }: BooleanCommandsProps) {
  const { availability, busy, notice, apply } = commands;
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const [stop, setStop] = useState(0);
  const [pressed, setPressed] = useState<BooleanOp | null>(null);

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
            <BooleanButton
              key={op}
              ref={(element) => {
                buttons.current[index] = element;
              }}
              op={op}
              availability={availability}
              selectTool={selectTool}
              tabIndex={index === stop ? 0 : -1}
              pressed={busy && pressed === op}
              onFocus={() => setStop(index)}
              onKeyDown={(event) => {
                const target = targetIndex(event.key, index);
                if (target !== null) {
                  event.preventDefault();
                  buttons.current[target]?.focus();
                }
              }}
              onActivate={(byMouse) => {
                if (busy) {
                  return;
                }
                setPressed(op);
                apply(op);
                if (byMouse) {
                  onReturnFocus();
                }
              }}
            />
          ))}
        </div>
        <BooleanNotice notice={notice} />
      </div>
    </>
  );
}
