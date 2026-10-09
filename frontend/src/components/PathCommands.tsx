import { useRef, useState } from "react";

import { CommandButton } from "@/components/CommandButton";
import { PathGlyph } from "@/components/PathGlyphs";
import {
  COLUMN_B_TOOLTIP_OFFSET_PX,
  RAIL_CARD_CLASS,
  RAIL_CARD_STYLE,
} from "@/components/railCard";
import type { PathCommands as PathCommandsState } from "@/hooks/useRailCommands";
import {
  commandDimmed,
  commandName,
  commandNote,
  commandRule,
  PATH_COMMANDS,
  type PathCommand,
} from "@/lib/pathText";

interface PathCommandsProps {
  commands: PathCommandsState;
  /** Whether the Select tool is active: with another tool the buttons are dimmed and the
   * tooltip says which tool to take. */
  selectTool: boolean;
  /** Called after a mouse press, so the tool letters keep working; a key press leaves the
   * focus on the button (`0016` criterion 22a). */
  onReturnFocus: () => void;
}

/** The index a navigation key moves the focus to (no wrap), or `null` for any other key. */
function targetIndex(key: string, index: number): number | null {
  const last = PATH_COMMANDS.length - 1;
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
 * The Path card of the tool rail (`docs/design-system.md`, row "Path toolbox"): the first card of
 * column B, Combine above Break apart, commands and not tools. One Tab stop with roving focus. It
 * shows what the session says is available and decides nothing. Its notice is drawn by the rail.
 */
export function PathCommands({ commands, selectTool, onReturnFocus }: PathCommandsProps) {
  const { availability, busy, apply } = commands;
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const [stop, setStop] = useState(0);
  const [pressed, setPressed] = useState<PathCommand | null>(null);

  return (
    <div
      role="toolbar"
      aria-label="Path operations"
      aria-orientation="vertical"
      aria-busy={busy}
      className={RAIL_CARD_CLASS}
      style={RAIL_CARD_STYLE}
    >
      {PATH_COMMANDS.map((command, index) => (
        <CommandButton
          key={command}
          ref={(element) => {
            buttons.current[index] = element;
          }}
          name={commandName(command)}
          rule={commandRule(command)}
          note={commandNote(command, availability, selectTool)}
          glyph={<PathGlyph command={command} />}
          dimmed={commandDimmed(command, availability)}
          tooltipOffset={COLUMN_B_TOOLTIP_OFFSET_PX}
          tabIndex={index === stop ? 0 : -1}
          pressed={busy && pressed === command}
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
            setPressed(command);
            apply(command);
            if (byMouse) {
              onReturnFocus();
            }
          }}
        />
      ))}
    </div>
  );
}
