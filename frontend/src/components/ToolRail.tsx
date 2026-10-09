import {
  Circle as CircleIcon,
  MousePointer2,
  MousePointer as SelectIcon,
  PenTool as PenToolIcon,
  Square,
  Star,
} from "lucide-react";
import { Tooltip } from "radix-ui";

import { BooleanCommands } from "@/components/BooleanCommands";
import { BooleanNotice } from "@/components/BooleanNotice";
import { RAIL_CARD_CLASS, RAIL_CARD_STYLE } from "@/components/railCard";
import type { BooleanCommands as BooleanCommandsState } from "@/hooks/useBooleanCommands";
import type { Tool } from "@/hooks/useEditorSession";

interface ToolRailProps {
  tool: Tool;
  /** How many objects are selected; with the Select tool active and one or
   * more selected the plain letters R and S act on the selection, so the
   * Rectangle tool is reached by "Esc, R" (`specs/0010-edit-interaction-polish/`
   * criterion 62). */
  selectionCount: number;
  onSelect: (tool: Tool) => void;
  /** Called after a mouse click on a button, so the tool letters keep working
   * (keyboard activation keeps focus on the rail for Tab navigation). */
  onReturnFocus: () => void;
  /** The Boolean toolbox below the tools card (`specs/0016-boolean-operations/`). */
  booleans: BooleanCommandsState;
}

interface ToolButtonProps {
  tool: Tool;
  active: boolean;
  label: string;
  shortcut: string;
  /** An optional second tooltip line (the modifiers of the tool). */
  hint?: string;
  icon: React.ReactNode;
  onSelect: (tool: Tool) => void;
  onReturnFocus: () => void;
}

/**
 * One tool rail button: a real focusable control, tab-reachable, with
 * an `aria-label` and a hover tooltip naming the shortcut
 * (`specification.md`'s UX notes, "Tool rail and tool switching").
 */
function ToolButton({
  tool,
  active,
  label,
  shortcut,
  hint,
  icon,
  onSelect,
  onReturnFocus,
}: ToolButtonProps) {
  return (
    <Tooltip.Root delayDuration={400}>
      <Tooltip.Trigger asChild>
        <button
          type="button"
          aria-label={`${label} (${shortcut})`}
          aria-pressed={active}
          onClick={(event) => {
            onSelect(tool);
            // `detail` is 0 for keyboard activation, 1 or more for a mouse click.
            if (event.detail > 0) {
              onReturnFocus();
            }
          }}
          className="flex size-10 items-center justify-center rounded-md outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-[var(--editor-accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--toolbar-bg)]"
          style={{
            background: active ? "var(--toolbar-icon-active-bg)" : "transparent",
            color: active ? "var(--toolbar-icon-active-fg)" : "var(--toolbar-icon)",
          }}
        >
          {icon}
        </button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="right"
          sideOffset={6}
          className="z-50 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground ring-1 ring-foreground/10"
        >
          <div>
            {label} ({shortcut})
          </div>
          {hint && <div>{hint}</div>}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

/**
 * The tool rail (`docs/design-system.md`, rows "Tool rail architecture", "Toolbox card" and
 * "Boolean toolbox"): one column of cards that floats over the canvas, inset 12px from the left
 * and top edges (`--rail-inset`), 48px wide (`--rail-right` is its right edge): the tools card,
 * then 8px below it the Boolean card. Select is first, a deliberate, one-time exception to "new
 * tools append in ship order" (acceptance criterion 12), then Pen, Node, Rectangle, Ellipse,
 * Polygon-star. The canvas is never resized or repositioned to make room for it.
 *
 * The root keeps 4px clear under the column. In a viewport too short for it the tools card stays
 * fixed and the cards below scroll (`0016` criterion 1b); the root passes no pointer event
 * through to itself, so the canvas around the cards stays usable.
 */
export function ToolRail({
  tool,
  selectionCount,
  onSelect,
  onReturnFocus,
  booleans,
}: ToolRailProps) {
  // The one state that changes a letter: the Select tool with a selection.
  const letterActsOnSelection = tool === "select" && selectionCount > 0;
  return (
    <Tooltip.Provider>
      <div className="pointer-events-none absolute top-3 bottom-1 left-3 z-30 flex w-12 flex-col gap-2">
        <div className={RAIL_CARD_CLASS} style={RAIL_CARD_STYLE}>
        <ToolButton
          tool="select"
          active={tool === "select"}
          label="Select tool"
          shortcut="S or Esc"
          hint="Drag a box. Shift: add. Ctrl: remove. Alt: invert, lasso, cycle"
          icon={<SelectIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="pen"
          active={tool === "pen"}
          label="Pen tool"
          shortcut="B"
          icon={<PenToolIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="node"
          active={tool === "node"}
          label="Node tool"
          shortcut="N"
          hint="Drag a segment to bend it. Shift: one axis"
          icon={<MousePointer2 size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="rectangle"
          active={tool === "rectangle"}
          label="Rectangle tool"
          shortcut={letterActsOnSelection ? "Esc, R" : "R"}
          hint="Shift: from centre. Ctrl: square or circle"
          icon={<Square size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="ellipse"
          active={tool === "ellipse"}
          label="Ellipse tool"
          shortcut="E"
          hint="Shift: from centre. Ctrl: square or circle"
          icon={<CircleIcon size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        <ToolButton
          tool="polygon-star"
          active={tool === "polygon-star"}
          label="Polygon/star tool"
          shortcut="*"
          icon={<Star size={20} />}
          onSelect={onSelect}
          onReturnFocus={onReturnFocus}
        />
        </div>
        {/* The cards below the tools card scroll in a viewport too short for them; the wider
         * box leaves room for their shadows. */}
        <div
          className="pointer-events-none -mx-3 min-h-0 overflow-y-auto px-3"
          style={{ scrollbarWidth: "thin" }}
        >
          <BooleanCommands
            commands={booleans}
            selectTool={tool === "select"}
            onReturnFocus={onReturnFocus}
          />
        </div>
        <BooleanNotice notice={booleans.notice} />
      </div>
    </Tooltip.Provider>
  );
}
