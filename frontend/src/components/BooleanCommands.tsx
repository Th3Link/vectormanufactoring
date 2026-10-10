import { useRef, useState } from "react";

import { BooleanGlyph } from "@/components/BooleanGlyphs";
import { CommandButton } from "@/components/CommandButton";
import {
  COLUMN_A_TOOLTIP_OFFSET_PX,
  RAIL_CARD_CLASS,
  RAIL_CARD_STYLE,
} from "@/components/railCard";
import type { BooleanCommands as BooleanCommandsState } from "@/hooks/useRailCommands";
import {
  BOOLEAN_OPS,
  type BooleanOp,
  operationName,
  operationRule,
  tooltipNote,
} from "@/lib/booleanText";

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
 * The Boolean toolbox of the tool rail (`docs/design-system.md`, rows "Boolean toolbox" and
 * "Toolbox card"): a card of its own below the tools card, five commands, not tools, one
 * column. One Tab stop with roving focus. It shows what the session says is available and
 * decides nothing. Its notice is drawn by the rail (`BooleanNotice`), outside the card's
 * scrolling column.
 */
export function BooleanCommands({ commands, selectTool, onReturnFocus }: BooleanCommandsProps) {
  const { availability, busy, apply } = commands;
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const [stop, setStop] = useState(0);
  const [pressed, setPressed] = useState<BooleanOp | null>(null);

  return (
    <div
      role="toolbar"
      aria-label="Boolean operations"
      aria-orientation="vertical"
      aria-busy={busy}
      className={RAIL_CARD_CLASS}
      style={RAIL_CARD_STYLE}
    >
      {BOOLEAN_OPS.map((op, index) => (
        <CommandButton
          key={op}
          ref={(element) => {
            buttons.current[index] = element;
          }}
          name={operationName(op)}
          rule={operationRule(op)}
          note={tooltipNote(availability, selectTool)}
          glyph={<BooleanGlyph op={op} />}
          dimmed={availability.needsTwo}
          tooltipOffset={COLUMN_A_TOOLTIP_OFFSET_PX}
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
  );
}
