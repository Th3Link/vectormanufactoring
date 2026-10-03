import { Diamond, Plus, Spline, Square, Trash2, Minus as MakeLineIcon } from "lucide-react";
import { ContextMenu } from "radix-ui";
import type { ReactNode } from "react";

import type { NodeToolbarState } from "@/hooks/useEditorSession";

export interface NodeActions {
  insertSelected: () => void;
  deleteSelected: () => void;
  convertSelected: (kind: "corner" | "smooth") => void;
  makeLine: () => void;
  makeCurve: () => void;
}

interface ActionDescriptor {
  key: string;
  label: string;
  icon: ReactNode;
  enabled: (state: NodeToolbarState) => boolean;
  run: (actions: NodeActions) => void;
}

/**
 * The node tool's six actions (`specification.md`'s UX notes,
 * "Node-tool actions"), shared between the contextual toolbar and the
 * right-click context menu so the two never drift apart — "belt-and-
 * suspenders discoverability", same six buttons either way.
 */
const ACTIONS: ActionDescriptor[] = [
  {
    key: "insert",
    label: "Insert node",
    icon: <Plus size={16} />,
    enabled: (state) => state.canInsert,
    run: (actions) => actions.insertSelected(),
  },
  {
    key: "delete",
    label: "Delete node",
    icon: <Trash2 size={16} />,
    enabled: (state) => state.canDelete,
    run: (actions) => actions.deleteSelected(),
  },
  {
    key: "make-corner",
    label: "Make corner",
    icon: <Square size={16} />,
    enabled: (state) => state.canConvertToCorner,
    run: (actions) => actions.convertSelected("corner"),
  },
  {
    key: "make-smooth",
    label: "Make smooth",
    icon: <Diamond size={16} />,
    enabled: (state) => state.canConvertToSmooth,
    run: (actions) => actions.convertSelected("smooth"),
  },
  {
    key: "make-line",
    label: "Make line",
    icon: <MakeLineIcon size={16} />,
    enabled: (state) => state.canMakeLine,
    run: (actions) => actions.makeLine(),
  },
  {
    key: "make-curve",
    label: "Make curve",
    icon: <Spline size={16} />,
    enabled: (state) => state.canMakeCurve,
    run: (actions) => actions.makeCurve(),
  },
];

interface NodeToolbarProps {
  state: NodeToolbarState;
  actions: NodeActions;
}

/**
 * The contextual tool-controls bar (`specification.md`'s UX notes):
 * shown directly under the main menu, full width, only while the node
 * tool is active. Buttons disable (not hide) when inapplicable.
 */
export function NodeToolbar({ state, actions }: NodeToolbarProps) {
  return (
    <div
      className="flex h-9 shrink-0 items-center gap-1 border-b border-border px-2"
      style={{ background: "var(--toolbar-bg)" }}
    >
      {ACTIONS.map((action) => (
        <button
          key={action.key}
          type="button"
          aria-label={action.label}
          title={action.label}
          disabled={!action.enabled(state)}
          onClick={() => action.run(actions)}
          className="flex size-7 items-center justify-center rounded-md text-[var(--toolbar-icon)] outline-none hover:bg-[var(--editor-accent-hover)] focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40"
        >
          {action.icon}
        </button>
      ))}
    </div>
  );
}

interface NodeContextMenuProps {
  state: NodeToolbarState;
  actions: NodeActions;
  children: ReactNode;
  /** The node tool isn't active — the trigger stays mounted (so the
   * canvas it wraps never remounts and loses its attached `wgpu`
   * surface, see this component's own doc comment) but won't open. */
  disabled: boolean;
}

/**
 * The same six actions on a right-click context menu over a selected
 * node/segment ("belt-and-suspenders discoverability, no memorization
 * required", same UX notes). Wraps the canvas: right-clicking anywhere
 * on it opens the menu, with each item's enabled state mirroring the
 * toolbar's.
 *
 * Always mounted around the canvas, `disabled` rather than conditionally
 * rendered: this wrapper sits between the canvas and its parent in the
 * DOM, so swapping it in/out by tool would move the `<canvas>` to a
 * different position in the tree on every switch — React then remounts
 * it as a brand-new element, and the `WasmSession`'s attached `wgpu`
 * surface (tied to the old, now-detached element) goes dark. Keeping one
 * stable wrapper, toggling only `disabled`, is what keeps the canvas
 * itself unmounted exactly once, for the component's whole lifetime.
 */
export function NodeContextMenu({ state, actions, children, disabled }: NodeContextMenuProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild disabled={disabled}>
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="z-50 min-w-40 rounded-md bg-popover p-1 text-popover-foreground ring-1 ring-foreground/10">
          {ACTIONS.map((action) => (
            <ContextMenu.Item
              key={action.key}
              disabled={!action.enabled(state)}
              onSelect={() => action.run(actions)}
              className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none data-disabled:pointer-events-none data-disabled:opacity-40 data-highlighted:bg-muted"
            >
              {action.icon}
              {action.label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
